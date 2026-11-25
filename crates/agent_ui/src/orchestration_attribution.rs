//! Who wrote a message of an orchestrated thread, for the `From <title>` and
//! `To <title>` headers that the thread view draws above it. The same layout
//! shows the messages between a Claude Code agent and its own subagents.
//!
//! Incoming messages carry their author: the orchestration tools tag every
//! message they deliver (see `MessageOrigin`), and the tag is stored on the
//! user message entry. Outgoing messages are derived from the entries: the
//! `send_to_parent` and `send_to_sibling` tool calls, and the final message of a
//! turn that the orchestrator started. Titles are looked up by thread id each
//! time they are drawn, so a rename shows at once.

use acp_thread::{
    AgentThreadEntry, AttributionRole, MessageAttribution, ToolCall, ToolCallStatus, UserMessage,
};
use agent_client_protocol::v2::schema as acp_v2;
use gpui::{App, SharedString};
use orchestration::{
    CHILD_HEADER_MARKER, OrchestrationRegistry, SenderRole, ThreadId, split_delivered_text,
};

use crate::{
    conversation_view::MessageOrigin, orchestration_host::child_conversation_view,
    thread_metadata_store::ThreadMetadataStore,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttributionDirection {
    From,
    To,
}

/// The header of an entry: `From <title>` or `To <title>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntryAttribution {
    pub direction: AttributionDirection,
    pub role: AttributionRole,
    pub title: SharedString,
}

pub(crate) fn attribution_role(role: SenderRole) -> AttributionRole {
    match role {
        SenderRole::Orchestrator => AttributionRole::Orchestrator,
        SenderRole::Sibling => AttributionRole::Sibling,
        SenderRole::Child => AttributionRole::Child,
    }
}

fn attribution_for(role: AttributionRole, thread: ThreadId) -> MessageAttribution {
    MessageAttribution {
        role,
        thread_key: thread.to_key_string().into(),
    }
}

/// The orchestrator that started `thread`, if it is an orchestrated child.
pub(crate) fn orchestrator_of(thread: ThreadId, cx: &App) -> Option<ThreadId> {
    ThreadMetadataStore::try_global(cx)
        .and_then(|store| store.read(cx).entry(thread)?.parent_thread_id)
        .or_else(|| {
            OrchestrationRegistry::try_global(cx)?
                .read(cx)
                .orchestrator_of(thread)
        })
}

/// The attribution to store on the user message that `thread` is about to send.
/// A child's first message is the prompt its orchestrator started it with, even
/// though it goes through the same path as a message typed by a person.
pub(crate) fn message_attribution(
    thread: ThreadId,
    origin: MessageOrigin,
    is_first_message: bool,
    cx: &App,
) -> Option<MessageAttribution> {
    match origin {
        MessageOrigin::Orchestration(Some(from)) => {
            Some(attribution_for(attribution_role(from.role), from.thread))
        }
        MessageOrigin::Orchestration(None) => None,
        MessageOrigin::Human if is_first_message => orchestrator_of(thread, cx)
            .map(|orchestrator| attribution_for(AttributionRole::Orchestrator, orchestrator)),
        MessageOrigin::Human => None,
    }
}

/// The current title of a thread, by id. Falls back to the live conversation
/// for a thread whose metadata is not saved yet.
pub(crate) fn thread_title(key: &str, cx: &App) -> SharedString {
    let Some(thread) = ThreadId::from_key_string(key) else {
        return crate::DEFAULT_THREAD_TITLE.into();
    };
    ThreadMetadataStore::try_global(cx)
        .and_then(|store| store.read(cx).entry(thread)?.title())
        .or_else(|| {
            let view = child_conversation_view(thread, cx)?;
            let thread = view.read(cx).root_thread(cx)?;
            thread.read(cx).title()
        })
        .unwrap_or_else(|| crate::DEFAULT_THREAD_TITLE.into())
}

fn first_text(blocks: &[acp_v2::ContentBlock]) -> Option<&str> {
    match blocks.first()? {
        acp_v2::ContentBlock::Text(text) => Some(&text.text),
        _ => None,
    }
}

/// The attribution of a user message: the stored one, or for a child's first
/// message after a restart (the stored one is not saved) the orchestrator that
/// the R14 header names.
pub(crate) fn user_message_attribution(
    message: &UserMessage,
    thread: ThreadId,
    cx: &App,
) -> Option<MessageAttribution> {
    if let Some(attribution) = &message.attribution {
        return Some(attribution.clone());
    }
    first_text(message.content.source_blocks())
        .filter(|text| text.starts_with(CHILD_HEADER_MARKER))?;
    orchestrator_of(thread, cx)
        .map(|orchestrator| attribution_for(AttributionRole::Orchestrator, orchestrator))
}

/// Whether another thread wrote the message. Such a message is read-only: it
/// is not something the person can edit and resend.
pub(crate) fn is_delivered(message: &UserMessage) -> bool {
    message.attribution.is_some()
        || first_text(message.content.source_blocks())
            .is_some_and(|text| text.starts_with(CHILD_HEADER_MARKER))
}

/// The blocks to show in the body of a message that another thread wrote: the
/// text without the part only the model needs (the R14 header, the prefix, how
/// to answer a sibling).
pub(crate) fn display_blocks(message: &UserMessage) -> Vec<acp_v2::ContentBlock> {
    let blocks = message.content.source_blocks();
    let mut blocks = blocks.to_vec();
    let Some(text) = first_text(&blocks) else {
        return blocks;
    };
    if !is_delivered(message) {
        return blocks;
    }
    let body = split_delivered_text(text).body.to_string();
    blocks[0] = acp_v2::ContentBlock::Text(acp_v2::TextContent::new(body));
    blocks
}

/// The part of a message that only the model needs, shown de-emphasized.
pub(crate) fn context_text(message: &UserMessage) -> Option<String> {
    if !is_delivered(message) {
        return None;
    }
    split_delivered_text(first_text(message.content.source_blocks())?)
        .context
        .map(|context| context.to_string())
}

/// Whether `name` is the orchestration tool `tool`, with or without the prefix
/// that an agent gives the tools of an MCP server (`mcp__zed__send_to_parent`).
fn is_tool(name: &str, tool: &str) -> bool {
    name.strip_suffix(tool).is_some_and(|prefix| {
        prefix
            .chars()
            .last()
            .is_none_or(|last| !last.is_alphanumeric())
    })
}

/// What a `send_to_parent` or `send_to_sibling` call sent, and to whom.
pub(crate) struct OutgoingMessage {
    pub role: AttributionRole,
    pub recipient: SharedString,
    pub text: String,
}

pub(crate) fn outgoing_message(
    tool_call: &ToolCall,
    thread: ThreadId,
    cx: &App,
) -> Option<OutgoingMessage> {
    // A call that failed or is waiting shows as the tool call it is, so the
    // error stays visible.
    if !matches!(
        tool_call.status(),
        ToolCallStatus::Pending | ToolCallStatus::InProgress | ToolCallStatus::Completed
    ) {
        return None;
    }
    let name = tool_call
        .tool_name
        .as_ref()
        .or(tool_call.name.as_ref())?
        .as_ref();
    let input = tool_call.raw_input.as_ref()?;
    let text = input.get("message")?.as_str()?.to_string();
    if is_tool(name, "send_to_parent") {
        let orchestrator = orchestrator_of(thread, cx)?;
        Some(OutgoingMessage {
            role: AttributionRole::Orchestrator,
            recipient: orchestrator.to_key_string().into(),
            text,
        })
    } else if is_tool(name, "send_to_sibling") {
        let sibling = ThreadId::from_key_string(input.get("child_id")?.as_str()?)?;
        Some(OutgoingMessage {
            role: AttributionRole::Sibling,
            recipient: sibling.to_key_string().into(),
            text,
        })
    } else {
        None
    }
}

/// The header of the entry at `entry_ix` of `thread`'s transcript, if another
/// thread wrote it or it is addressed to another thread.
pub(crate) fn entry_attribution(
    entries: &[AgentThreadEntry],
    entry_ix: usize,
    is_generating: bool,
    thread: ThreadId,
    cx: &App,
) -> Option<EntryAttribution> {
    match entries.get(entry_ix)? {
        AgentThreadEntry::UserMessage(message) => {
            let attribution = user_message_attribution(message, thread, cx)?;
            Some(EntryAttribution {
                direction: AttributionDirection::From,
                role: attribution.role,
                title: thread_title(&attribution.thread_key, cx),
            })
        }
        AgentThreadEntry::ToolCall(tool_call) => {
            let outgoing = outgoing_message(tool_call, thread, cx)?;
            Some(EntryAttribution {
                direction: AttributionDirection::To,
                role: outgoing.role,
                title: thread_title(&outgoing.recipient, cx),
            })
        }
        AgentThreadEntry::AssistantMessage(_) => {
            // The final message of a turn the orchestrator started is what the
            // orchestrator receives.
            let user_ix = entries[..entry_ix]
                .iter()
                .rposition(|entry| matches!(entry, AgentThreadEntry::UserMessage(_)))?;
            let AgentThreadEntry::UserMessage(user) = &entries[user_ix] else {
                return None;
            };
            let attribution = user_message_attribution(user, thread, cx)?;
            if attribution.role != AttributionRole::Orchestrator {
                return None;
            }
            let rest = &entries[entry_ix + 1..];
            let turn_continues = rest
                .iter()
                .take_while(|entry| !matches!(entry, AgentThreadEntry::UserMessage(_)))
                .any(|entry| matches!(entry, AgentThreadEntry::AssistantMessage(_)));
            let turn_is_open = is_generating
                && !rest
                    .iter()
                    .any(|entry| matches!(entry, AgentThreadEntry::UserMessage(_)));
            if turn_continues || turn_is_open {
                return None;
            }
            Some(EntryAttribution {
                direction: AttributionDirection::To,
                role: AttributionRole::Orchestrator,
                title: thread_title(&attribution.thread_key, cx),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tool_accepts_mcp_prefixes_only() {
        assert!(is_tool("send_to_parent", "send_to_parent"));
        assert!(is_tool("mcp__zed__send_to_parent", "send_to_parent"));
        assert!(is_tool("zed/send_to_parent", "send_to_parent"));
        assert!(!is_tool("resend_to_parent", "send_to_parent"));
        assert!(!is_tool("send_to_parent_later", "send_to_parent"));
    }
}
