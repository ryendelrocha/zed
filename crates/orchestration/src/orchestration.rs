//! Orchestrated threads: one thread (the orchestrator) starts other threads
//! (children), follows them, and receives only each child's final message.
//!
//! This crate holds the parts that do not depend on the UI:
//!
//! - the input and output types of the orchestration tools,
//! - [`OrchestrationRegistry`], a global that tracks every child of every
//!   orchestrator and mints per-session tokens that identify an orchestrator,
//! - [`ChildLog`], the JSONL log of the orchestrator-child exchanges,
//! - the [`OrchestrationHost`] trait, implemented by the agent panel.
//!
//! Tool adapters (native tools, the MCP server) are thin layers over
//! [`OrchestrationHost`]. They identify the orchestrator with a [`ThreadId`]:
//! an MCP caller resolves its token with
//! [`OrchestrationRegistry::resolve_token`], and a native tool resolves its
//! session id with [`OrchestrationRegistry::host_for_session`].

mod bridge;
mod child_log;
mod mcp;
mod registry;

use std::fmt;

use anyhow::Result;
use gpui::{App, SharedString, Task};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlez::{
    bindable::{Bind, Column, StaticColumnCount},
    statement::Statement,
};

pub use bridge::*;
pub use child_log::*;
pub use mcp::*;
pub use registry::*;

/// Identifies a thread in the agent panel and the threads sidebar. It is the
/// stable id of both orchestrators and children.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ThreadId(uuid::Uuid);

impl ThreadId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Stable, hyphenated string form suitable for use as a key. This is the
    /// `child_id` that the tools hand to the model.
    pub fn to_key_string(&self) -> String {
        self.0.hyphenated().to_string()
    }

    /// Parses the string produced by [`ThreadId::to_key_string`].
    pub fn from_key_string(key: &str) -> Option<Self> {
        uuid::Uuid::parse_str(key.trim()).ok().map(Self)
    }

    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> uuid::Uuid {
        self.0
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0.hyphenated())
    }
}

impl StaticColumnCount for ThreadId {}

impl Bind for ThreadId {
    fn bind(&self, statement: &Statement, start_index: i32) -> Result<i32> {
        self.0.bind(statement, start_index)
    }
}

impl Column for ThreadId {
    fn column(statement: &mut Statement, start_index: i32) -> Result<(Self, i32)> {
        let (uuid, next) = Column::column(statement, start_index)?;
        Ok((ThreadId(uuid), next))
    }
}

/// Lifecycle of a child thread, as the orchestrator sees it.
///
/// A child can leave a terminal state: `send_to_child` to a completed child
/// starts a new turn and moves it back to `Running`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChildStatus {
    /// The thread was created and is not generating yet (for example, the
    /// agent is still connecting).
    Starting,
    /// The child is generating.
    Running,
    /// The child asked for permission and is waiting for an answer.
    WaitingForConfirmation,
    /// The child finished its turn.
    Completed,
    /// The child stopped with an error or a refusal, or its connection was lost.
    Failed,
    /// The child was cancelled.
    Cancelled,
}

impl ChildStatus {
    /// Whether `await_children` stops waiting on a child in this state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// Where a child works.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeMode {
    /// The child works in the orchestrator's worktree. Cheaper, but the
    /// children running at the same time must touch disjoint files.
    #[default]
    Shared,
    /// The child gets its own git worktree, in a new workspace.
    New,
}

/// Starts a child thread and returns right away, without waiting for it.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct StartChildInput {
    /// Short title of the child thread, shown in the threads sidebar.
    pub title: String,
    /// The first message of the child. It must contain everything the child
    /// needs, because the child does not see the orchestrator's conversation.
    pub prompt: String,
    /// Agent that runs the child, as listed by `list_agents_and_models`.
    /// Defaults to the agent selected in the agent panel.
    #[serde(default)]
    pub agent: Option<String>,
    /// Model of the child, as `provider/model-id`. Only the native Zed agent
    /// supports it for now: for an external agent it is ignored and the
    /// output carries a warning.
    #[serde(default)]
    pub model: Option<String>,
    /// Whether the child shares the orchestrator's worktree or gets its own.
    #[serde(default)]
    pub worktree: WorktreeMode,
    /// With `worktree: new`, the git ref to base the new worktree on.
    /// Defaults to the current HEAD.
    #[serde(default)]
    pub base_ref: Option<String>,
    /// With `worktree: new`, a name for the new worktree.
    #[serde(default)]
    pub worktree_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct StartChildOutput {
    /// Identifier to use in the other orchestration tools.
    pub child_id: String,
    /// Path of the JSONL file with the orchestrator-child exchanges.
    pub log_path: String,
    /// Agent that runs the child.
    pub agent: String,
    /// Something the orchestrator should know, for example that the model was
    /// not applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Waits until children finish their turn, or until the timeout, and returns
/// each child's final message only.
///
/// Read the whole result, not only `final_message`: `pending_messages` holds
/// what a child told you with `send_to_parent` (a blocker, a question), and
/// `human_interventions` says that a human stepped in on a child, what they
/// changed and why. Both are returned once. A child that sends an urgent
/// message ends the wait early, with the other children still running.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct AwaitChildrenInput {
    /// Children to wait for. An empty list waits for every child of this
    /// orchestrator.
    #[serde(default)]
    pub child_ids: Vec<String>,
    /// How long to wait, in seconds. Defaults to 120. When it expires, the
    /// children that are still running are returned with their current
    /// status and no message; call the tool again to keep waiting.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ChildResult {
    pub child_id: String,
    pub title: String,
    pub status: ChildStatus,
    /// The last message the child wrote in its latest turn. It is present
    /// only when the child finished (`completed`, `failed` or `cancelled`),
    /// and it is never the child's transcript.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_message: Option<String>,
    /// Messages the child sent with `send_to_parent` that you have not seen
    /// yet. Each is returned once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_messages: Vec<PendingMessage>,
    /// Summaries of what a human changed by writing directly in the child
    /// since the last summary. Each is returned once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub human_interventions: Vec<HumanIntervention>,
}

/// A message a child sent to its orchestrator with `send_to_parent`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PendingMessage {
    pub message: String,
    /// RFC 3339 time the child sent it.
    pub sent_at: String,
    /// Whether the child marked it urgent.
    pub urgent: bool,
}

/// What a human did in a child thread, summarized once for the orchestrator.
/// The human's own messages are never part of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HumanIntervention {
    /// That a human intervened, what they adjusted and why.
    pub summary: String,
    /// How many human-driven turns the summary covers.
    pub turns: u32,
    /// RFC 3339 time the summary was written.
    pub summarized_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AwaitChildrenOutput {
    pub children: Vec<ChildResult>,
    /// True when the timeout expired before every child finished.
    pub timed_out: bool,
}

/// Reports where children are, without any of their content. Like
/// `await_children`, it also returns each child's `pending_messages` and
/// `human_interventions`, once. Check them: a child may be waiting on an answer
/// from you, or a human may have changed its course.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct ChildStatusInput {
    /// Children to report. An empty list reports every child of this
    /// orchestrator.
    #[serde(default)]
    pub child_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ChildStatusInfo {
    pub child_id: String,
    pub title: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub status: ChildStatus,
    /// Number of turns the child has completed.
    pub turns: u32,
    /// RFC 3339 time of the last exchange or status change.
    pub last_activity: String,
    /// Messages the child sent with `send_to_parent` that you have not seen
    /// yet. Each is returned once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_messages: Vec<PendingMessage>,
    /// Summaries of what a human changed by writing directly in the child
    /// since the last summary. Each is returned once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub human_interventions: Vec<HumanIntervention>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ChildStatusOutput {
    pub children: Vec<ChildStatusInfo>,
}

/// Sends a follow-up message to a child. If the child is in the middle of a
/// turn, the message is queued and delivered when the turn ends.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SendToChildInput {
    pub child_id: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SendToChildOutput {
    /// True when the child was busy and the message was queued.
    pub queued: bool,
}

/// Sends a message to the orchestrator that started this thread.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SendToParentInput {
    /// What the orchestrator needs to know: a blocker, a question, a decision
    /// you need. Be specific and self-contained.
    pub message: String,
    /// Set it only when you are blocked and cannot continue without an
    /// answer. The message is then also added to the orchestrator's thread as
    /// a new message, but only when the orchestrator is idle: it never
    /// interrupts a running turn. Defaults to false.
    #[serde(default)]
    pub urgent: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SendToParentOutput {
    pub parent_id: String,
    pub parent_title: String,
    pub urgent: bool,
}

/// Reports this thread's place in an orchestration.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct WhoAmIInput {}

/// How a thread takes part in an orchestration.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThreadRole {
    /// The thread started children.
    Orchestrator,
    /// An orchestrator started the thread.
    Child,
    /// The thread started children and was started by an orchestrator.
    Both,
    /// The thread is not part of an orchestration.
    None,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ThreadReference {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChildReference {
    pub id: String,
    pub title: String,
    pub status: ChildStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WhoAmIOutput {
    pub role: ThreadRole,
    /// The orchestrator that started this thread, if any. Reach it with
    /// `send_to_parent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<ThreadReference>,
    /// The children this thread started.
    pub children: Vec<ChildReference>,
}

/// What the host needs to summarize the human turns in a child.
#[derive(Clone, Debug)]
pub struct HumanSummaryRequest {
    pub child: ThreadId,
    /// How many human-driven turns happened since the previous summary.
    pub turns: u32,
    /// Index of the child's first thread entry the summary covers.
    pub from_entry: usize,
    /// Indexes of the user messages in that range that the orchestrator sent,
    /// not the human.
    pub orchestrator_entries: Vec<usize>,
    /// Cap on the length of the summary, in characters.
    pub max_chars: usize,
}

/// Prefix of a child's first prompt: it tells the child that it is a child,
/// who its orchestrator is and how to reach it.
pub fn child_header(orchestrator_title: &str) -> String {
    format!(
        "You are a child conversation started by the orchestrator thread \"{orchestrator_title}\". \
         When you finish, your final message goes back to the orchestrator automatically, so you do \
         not need to send it. If you need to reach the orchestrator during the task (a blocker, a \
         question, a decision), use the `send_to_parent` tool. Do not write it in your own chat: the \
         orchestrator cannot read your chat. Use `whoami` to see your role, your orchestrator and \
         your own children.\n\n---\n\n"
    )
}

/// Prefix of a message that `send_to_parent` adds to the orchestrator's thread.
pub fn urgent_message_prefix(child_title: &str) -> String {
    format!("[from child {child_title}]")
}

/// Prompt that asks a model for the summary of a human's turns in a child.
/// `transcript` holds only the turns the summary covers.
pub fn human_summary_prompt(transcript: &str, max_chars: usize) -> String {
    format!(
        "A human wrote directly in a child conversation that an orchestrator started. Their turns \
         are below, with the agent's replies. Write one short summary for the orchestrator that \
         says: that a human intervened, what they adjusted, and why. Describe the change, not the \
         process, and do not quote the human. Plain text, no preamble, at most {max_chars} \
         characters.\n\n<conversation>\n{transcript}\n</conversation>"
    )
}

/// Cuts `text` to at most `max_chars` characters, ending with an ellipsis when
/// it had to cut.
pub fn truncate_summary(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let mut cut: String = text.chars().take(keep).collect();
    cut.truncate(cut.trim_end().len());
    if max_chars > 0 {
        cut.push('\u{2026}');
    }
    cut
}

/// What the orchestrator gets when no summary model is available.
pub fn human_summary_fallback(turns: u32) -> String {
    let noun = if turns == 1 { "turn" } else { "turns" };
    format!("A human intervened in {turns} {noun}; no summary model configured.")
}

/// Cancels the current turn of a child.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CancelChildInput {
    pub child_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CancelChildOutput {
    pub status: ChildStatus,
}

/// The agents a child can run on, for `list_agents_and_models`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct AvailableAgents {
    pub agents: Vec<AvailableAgent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AvailableAgent {
    /// Value for the `agent` field of `start_child`.
    pub id: String,
    pub name: SharedString,
    /// Whether this is Zed's built-in agent.
    pub is_native: bool,
    /// May be empty for agents that choose their own models.
    pub models: Vec<AvailableModel>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AvailableModel {
    /// Value for the `model` field of `start_child`.
    pub id: String,
    pub name: SharedString,
    pub is_default: bool,
}

/// Default for [`AwaitChildrenInput::timeout_secs`]. It is deliberately below
/// the tool timeout of common MCP clients.
pub const DEFAULT_AWAIT_TIMEOUT_SECS: u64 = 120;

/// Upper bound applied to [`AwaitChildrenInput::timeout_secs`].
pub const MAX_AWAIT_TIMEOUT_SECS: u64 = 3600;

/// What the UI layer provides to the orchestration tools. The agent panel
/// implements it; the native tools and the MCP server call it.
///
/// Every method takes the `orchestrator` thread that is calling, so a child
/// can only be reached by the thread that started it. `child_status` and
/// `await_children` have default implementations over
/// [`OrchestrationRegistry`], because they need nothing from the UI.
pub trait OrchestrationHost {
    /// Creates a child thread linked to `orchestrator` and returns once the
    /// thread exists, without waiting for its first turn.
    fn start_child(
        &self,
        orchestrator: ThreadId,
        input: StartChildInput,
        cx: &mut App,
    ) -> Task<Result<StartChildOutput>>;

    /// Delivers a follow-up message to a child, queueing it when the child is
    /// mid-turn.
    fn send_to_child(
        &self,
        orchestrator: ThreadId,
        input: SendToChildInput,
        cx: &mut App,
    ) -> Task<Result<SendToChildOutput>>;

    fn cancel_child(
        &self,
        orchestrator: ThreadId,
        input: CancelChildInput,
        cx: &mut App,
    ) -> Task<Result<CancelChildOutput>>;

    fn list_available_agents(&self, cx: &mut App) -> Result<AvailableAgents>;

    /// Finds the thread whose root ACP session is `session_id`, if this host
    /// owns it. This is how a native tool, which knows only its own session,
    /// learns which orchestrator it is acting for.
    fn resolve_orchestrator(&self, session_id: &str, cx: &App) -> Option<ThreadId>;

    /// Whether this host owns the thread `thread_id`.
    fn owns_thread(&self, thread_id: ThreadId, cx: &App) -> bool;

    /// The current title of a thread this host owns.
    fn thread_title(&self, _thread: ThreadId, _cx: &App) -> Option<String> {
        None
    }

    /// Whether the thread is between turns. A message may be added to it
    /// without interrupting anything only while this is true.
    fn is_thread_idle(&self, _thread: ThreadId, _cx: &App) -> bool {
        false
    }

    /// Adds `text` to the thread as a new user message that starts a turn. It
    /// must never cancel a running turn.
    fn deliver_message(&self, _thread: ThreadId, _text: String, _cx: &mut App) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "This host cannot deliver messages to threads."
        )))
    }

    /// Summarizes the human-driven turns of a child with the thread summary
    /// model. `Ok(None)` means no model is available.
    fn summarize_human_interventions(
        &self,
        _request: HumanSummaryRequest,
        _cx: &mut App,
    ) -> Task<Result<Option<String>>> {
        Task::ready(Ok(None))
    }

    fn child_status(
        &self,
        orchestrator: ThreadId,
        input: ChildStatusInput,
        cx: &mut App,
    ) -> Task<Result<ChildStatusOutput>> {
        let registry = OrchestrationRegistry::global_or_init(cx);
        OrchestrationRegistry::child_status_with_summaries(&registry, orchestrator, input, cx)
    }

    fn send_to_parent(
        &self,
        caller: ThreadId,
        input: SendToParentInput,
        cx: &mut App,
    ) -> Task<Result<SendToParentOutput>> {
        let registry = OrchestrationRegistry::global_or_init(cx);
        Task::ready(OrchestrationRegistry::send_to_parent(
            &registry, caller, input, cx,
        ))
    }

    fn whoami(&self, caller: ThreadId, cx: &mut App) -> WhoAmIOutput {
        let registry = OrchestrationRegistry::global_or_init(cx);
        registry.read(cx).whoami(caller, cx)
    }

    fn await_children(
        &self,
        orchestrator: ThreadId,
        input: AwaitChildrenInput,
        cx: &mut App,
    ) -> Task<Result<AwaitChildrenOutput>> {
        let registry = OrchestrationRegistry::global_or_init(cx);
        OrchestrationRegistry::await_children(&registry, orchestrator, input, cx)
    }
}
