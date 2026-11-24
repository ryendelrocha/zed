//! Subagent threads nested under their parent thread in the sidebar.
//!
//! Only live subagents are listed: they come from the parent's conversation
//! while it is loaded in the agent panel. Only direct children are shown, so a
//! subagent's own subagents never appear here.

use std::collections::{HashMap, HashSet};

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

/// What the sidebar remembers about subagent groups, and the settings they
/// are laid out with.
pub(crate) struct SubthreadLayout<'a> {
    pub groups: &'a HashMap<ThreadId, Vec<SubthreadInfo>>,
    pub collapsed: &'a HashSet<ThreadId>,
    pub fully_loaded: &'a HashSet<ThreadId>,
    pub visible: usize,
}

/// Inserts the subagent rows of every thread in `entries[start..]` right after
/// that thread, and records how many subagents each parent has in `counts`.
pub(crate) fn insert_subthread_entries(
    entries: &mut Vec<ListEntry>,
    start: usize,
    layout: &SubthreadLayout,
    counts: &mut HashMap<ThreadId, usize>,
) {
    if layout.groups.is_empty() {
        return;
    }
    let group_entries = entries.split_off(start);
    for entry in group_entries {
        let parent = match &entry {
            ListEntry::Thread(thread)
                if thread.draft.is_none()
                    && matches!(thread.workspace, ThreadEntryWorkspace::Open(_)) =>
            {
                Some(thread.clone())
            }
            _ => None,
        };
        entries.push(entry);

        let Some(parent) = parent else { continue };
        let thread_id = parent.metadata.thread_id;
        let Some(subthreads) = layout.groups.get(&thread_id).filter(|s| !s.is_empty()) else {
            continue;
        };
        counts.insert(thread_id, subthreads.len());
        if layout.collapsed.contains(&thread_id) {
            continue;
        }
        let VisibleSubthreads { shown, hidden } = select_visible_subthreads(
            subthreads.clone(),
            layout.visible,
            layout.fully_loaded.contains(&thread_id),
        );
        for info in shown {
            entries.push(ListEntry::Subthread(SubthreadEntry {
                parent: parent.clone(),
                info,
            }));
        }
        if hidden > 0 {
            entries.push(ListEntry::LoadMoreSubthreads {
                parent_thread_id: thread_id,
            });
        }
    }
}
