use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{ChildStatusInput, ChildStatusOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
Report where child threads are, without any of their content.

Returns, for each child, its title, agent, model, status (`starting`, `running`, `waiting_for_confirmation`, `completed`, `failed` or `cancelled`), the number of completed turns, the time of the last activity and `sibling_messages` (how many messages the child exchanged with its siblings; their content is not shown to you and stays in the child's log). It never returns the child's messages.

Also check these two fields, each returned once:
- `pending_messages`: what the child told you with `send_to_parent` (a blocker, a question). Answer with `send_to_child`.
- `human_interventions`: a human wrote directly in the child. Each entry is a short summary of what they changed and why, so you continue coherently. The human's own messages are never shown.

Use it for a quick look, for example to see whether a child is waiting for the user's permission or for your answer. To get results, use `await_children`, not repeated calls to this tool.

`child_ids` empty reports every child of this thread.";

pub struct ChildStatusTool {
    session_id: String,
}

impl ChildStatusTool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds its orchestrator at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for ChildStatusTool {
    type Input = ChildStatusInput;
    type Output = OrchestrationToolOutput<ChildStatusOutput>;

    const NAME: &'static str = "child_status";

    fn description() -> SharedString {
        DESCRIPTION.into()
    }

    fn kind() -> acp::ToolKind {
        acp::ToolKind::Other
    }

    fn initial_title(
        &self,
        input: Result<Self::Input, serde_json::Value>,
        _cx: &mut App,
    ) -> SharedString {
        let _ = input;
        "Check child status".into()
    }

    fn run(
        self: Arc<Self>,
        input: ToolInput<Self::Input>,
        _event_stream: ToolCallEventStream,
        cx: &mut App,
    ) -> Task<Result<Self::Output, Self::Output>> {
        run_orchestration_tool(
            self.session_id.clone(),
            input,
            cx,
            |host, orchestrator, input, cx| host.child_status(orchestrator, input, cx),
        )
    }
}
