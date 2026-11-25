//! Child threads nested under their parent thread in the sidebar.
//!
//! Two kinds of child are listed, one level deep:
//!
//! - live subagents, which come from the parent's conversation while it is
//!   loaded in the agent panel;
//! - threads of their own that the parent started (orchestrated threads),
//!   which come from the thread metadata through `parent_thread_id`, so they
//!   persist across restarts. They keep their ordinary thread row, indented,
//!   so they open, rename, archive and show status like any thread.
//!
//! A child whose parent has no row in the same list (archived, missing, or in
//! another project group) stays in the list as an ordinary thread, so it can
//! never disappear. A child of a child is not nested either.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agent_ui::{AgentPanel, SubthreadInfo, ThreadId};
use gpui::{App, Entity};
use workspace::Workspace;

use crate::{ListEntry, ThreadEntry, ThreadEntryWorkspace};

/// The live subagent threads of every conversation loaded in the agent panels
/// of `workspaces`, keyed by the thread that owns them.
pub(crate) fn live_subthreads_by_parent(
    workspaces: &[Entity<Workspace>],
    cx: &App,
) -> HashMap<ThreadId, Vec<SubthreadInfo>> {
    let mut groups = HashMap::new();
    for workspace in workspaces {
        let Some(agent_panel) = workspace.read(cx).panel::<AgentPanel>(cx) else {
            continue;
        };
        for conversation_view in agent_panel.read(cx).conversation_views() {
            let conversation_view = conversation_view.read(cx);
            let subthreads = conversation_view.subthreads(cx);
            if !subthreads.is_empty() {
                groups.insert(conversation_view.parent_id(), subthreads);
            }
        }
    }
    groups
}

/// A subagent thread row, listed under `parent`.
#[derive(Clone)]
pub(crate) struct SubthreadEntry {
    pub parent: std::sync::Arc<ThreadEntry>,
    pub info: SubthreadInfo,
}

/// The subagent rows to show for one parent thread.
#[derive(Debug, PartialEq)]
pub(crate) struct VisibleSubthreads {
    pub shown: Vec<SubthreadInfo>,
    pub hidden: usize,
}

/// Picks the subagent threads to list: the `visible` most recent ones, or all
/// of them once `show_all` is set. Most recent last message first; threads
/// that have no message yet go last.
pub(crate) fn select_visible_subthreads(
    mut subthreads: Vec<SubthreadInfo>,
    visible: usize,
    show_all: bool,
) -> VisibleSubthreads {
    subthreads.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));
    let limit = if show_all { subthreads.len() } else { visible };
    let hidden = subthreads.len().saturating_sub(limit);
    subthreads.truncate(limit);
    VisibleSubthreads {
        shown: subthreads,
        hidden,
    }
}

/// What the sidebar remembers about child groups, and the settings they are
/// laid out with.
pub(crate) struct SubthreadLayout<'a> {
    /// The live subagents of each loaded conversation.
    pub groups: &'a HashMap<ThreadId, Vec<SubthreadInfo>>,
    /// Whether threads with a `parent_thread_id` nest under their parent.
    pub nest_children: bool,
    pub collapsed: &'a HashSet<ThreadId>,
    pub fully_loaded: &'a HashSet<ThreadId>,
    pub visible: usize,
}

/// The row data of a thread that is a child in its own right.
fn child_info(child: &ThreadEntry) -> SubthreadInfo {
    let (icon, icon_from_external_svg) = match child.model.as_ref().and_then(|m| m.icon.as_ref()) {
        Some(model_icon) => agent_ui::model_icon_parts(model_icon),
        None => (child.icon, child.icon_from_external_svg.clone()),
    };
    SubthreadInfo {
        session_id: child
            .metadata
            .session_id
            .clone()
            .unwrap_or_else(|| agent_client_protocol::v2::schema::SessionId::new("")),
        title: child.metadata.display_title(),
        status: child.status,
        last_message_at: Some(child.metadata.updated_at),
        icon,
        icon_from_external_svg,
        model: child.model.as_ref().map(|model| model.name.clone()),
        thread_id: Some(child.metadata.thread_id),
    }
}

/// Inserts the child rows of every thread in `entries[start..]` right after
/// that thread, and records how many children each parent has in `counts`.
/// Threads that nest are taken out of their place in the list.
pub(crate) fn insert_subthread_entries(
    entries: &mut Vec<ListEntry>,
    start: usize,
    layout: &SubthreadLayout,
    counts: &mut HashMap<ThreadId, usize>,
) {
    if layout.groups.is_empty() && !layout.nest_children {
        return;
    }
    let group_entries = entries.split_off(start);

    // Only a thread with no parent of its own can have children: one level.
    let roots: HashSet<ThreadId> = if layout.nest_children {
        group_entries
            .iter()
            .filter_map(|entry| match entry {
                ListEntry::Thread(thread)
                    if thread.draft.is_none() && thread.metadata.parent_thread_id.is_none() =>
                {
                    Some(thread.metadata.thread_id)
                }
                _ => None,
            })
            .collect()
    } else {
        HashSet::new()
    };

    let mut children: HashMap<ThreadId, Vec<Arc<ThreadEntry>>> = HashMap::new();
    let mut top_level = Vec::with_capacity(group_entries.len());
    for entry in group_entries {
        match entry {
            ListEntry::Thread(thread)
                if thread.draft.is_none()
                    && thread.metadata.session_id.is_some()
                    && thread
                        .metadata
                        .parent_thread_id
                        .is_some_and(|parent| roots.contains(&parent)) =>
            {
                let parent = thread.metadata.parent_thread_id.expect("checked above");
                children.entry(parent).or_default().push(thread);
            }
            other => top_level.push(other),
        }
    }

    for entry in top_level {
        let parent = match &entry {
            ListEntry::Thread(thread) if thread.draft.is_none() => Some(thread.clone()),
            _ => None,
        };
        entries.push(entry);

        let Some(parent) = parent else { continue };
        let thread_id = parent.metadata.thread_id;

        let mut infos: Vec<SubthreadInfo> = Vec::new();
        if matches!(parent.workspace, ThreadEntryWorkspace::Open(_))
            && let Some(live) = layout.groups.get(&thread_id)
        {
            infos.extend(live.iter().cloned());
        }
        let mut child_entries: HashMap<ThreadId, Arc<ThreadEntry>> = HashMap::new();
        for mut child in children.remove(&thread_id).unwrap_or_default() {
            {
                let nested = Arc::make_mut(&mut child);
                nested.nested = true;
                let info = child_info(nested);
                nested.icon = info.icon;
                nested.icon_from_external_svg = info.icon_from_external_svg.clone();
            }
            infos.push(child_info(&child));
            child_entries.insert(child.metadata.thread_id, child);
        }
        if infos.is_empty() {
            continue;
        }

        counts.insert(thread_id, infos.len());
        if layout.collapsed.contains(&thread_id) {
            continue;
        }
        let VisibleSubthreads { shown, hidden } = select_visible_subthreads(
            infos,
            layout.visible,
            layout.fully_loaded.contains(&thread_id),
        );
        for info in shown {
            match info.thread_id.and_then(|id| child_entries.remove(&id)) {
                Some(child) => entries.push(ListEntry::Thread(child)),
                None => entries.push(ListEntry::Subthread(SubthreadEntry {
                    parent: parent.clone(),
                    info,
                })),
            }
        }
        if hidden > 0 {
            entries.push(ListEntry::LoadMoreSubthreads {
                parent_thread_id: thread_id,
            });
        }
    }
}
