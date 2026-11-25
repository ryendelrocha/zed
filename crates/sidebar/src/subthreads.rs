//! Child threads nested under their parent thread in the sidebar.
//!
//! Three kinds of child are listed, one level deep:
//!
//! - live subagents, which come from the parent's conversation while it is
//!   loaded in the agent panel (the only source for external agents such as
//!   Claude Code, whose subagents are not persisted);
//! - native-agent subagents persisted in the threads database with a parent
//!   session. Only their summaries are read, so a parent that was never
//!   opened still knows it has children;
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

use agent::ThreadStore;
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

/// The persisted native subagents, keyed by the session of their parent.
/// Built from the summaries the thread store already lists: no thread is
/// loaded.
pub(crate) fn persisted_subthreads_by_parent(
    cx: &App,
) -> HashMap<agent_client_protocol::v2::schema::SessionId, Vec<SubthreadInfo>> {
    let mut groups: HashMap<_, Vec<SubthreadInfo>> = HashMap::new();
    let Some(thread_store) = ThreadStore::try_global(cx) else {
        return groups;
    };
    for subagent in thread_store.read(cx).subagent_entries() {
        let Some(parent) = subagent.parent_session_id.clone() else {
            continue;
        };
        groups.entry(parent).or_default().push(SubthreadInfo {
            session_id: subagent.id.clone(),
            title: subagent.title.clone(),
            status: ui::AgentThreadStatus::Completed,
            last_message_at: Some(subagent.updated_at),
            icon: ui::IconName::ZedAgent,
            icon_from_external_svg: None,
            model: None,
            diff_stats: Default::default(),
            thread_id: None,
        });
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

/// Picks the subagent threads to list: the `count` most recent ones. Most
/// recent last message first; threads that have no message yet go last.
pub(crate) fn select_visible_subthreads(
    mut subthreads: Vec<SubthreadInfo>,
    count: usize,
) -> VisibleSubthreads {
    subthreads.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));
    let hidden = subthreads.len().saturating_sub(count);
    subthreads.truncate(count);
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
    /// The native subagents persisted for each parent session.
    pub persisted: &'a HashMap<agent_client_protocol::v2::schema::SessionId, Vec<SubthreadInfo>>,
    /// How many children each group shows, once "Load more" was used. A group
    /// that is not here shows `visible`. Kept across collapse and expand.
    pub shown_counts: &'a HashMap<ThreadId, usize>,
    /// The page size: how many children a group shows, and how many more each
    /// "Load more" reveals.
    pub visible: usize,
}

/// The row data of a thread that is a child in its own right.
pub(crate) fn child_info(child: &ThreadEntry) -> SubthreadInfo {
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
        diff_stats: child.diff_stats,
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
    if layout.groups.is_empty() && layout.persisted.is_empty() && !layout.nest_children {
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
        // Persisted native subagents, unless the loaded conversation already
        // lists them live.
        if let Some(parent_session_id) = parent.metadata.session_id.as_ref()
            && let Some(persisted) = layout.persisted.get(parent_session_id)
        {
            let live_sessions: HashSet<_> =
                infos.iter().map(|info| info.session_id.clone()).collect();
            infos.extend(
                persisted
                    .iter()
                    .filter(|info| !live_sessions.contains(&info.session_id))
                    .cloned(),
            );
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
        let count = layout
            .shown_counts
            .get(&thread_id)
            .copied()
            .unwrap_or(layout.visible);
        let VisibleSubthreads { shown, hidden } = select_visible_subthreads(infos, count);
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
