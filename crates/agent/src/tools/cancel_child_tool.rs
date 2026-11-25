use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{CancelChildInput, CancelChildOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
Cancel the current turn of a child thread you started.

Use it when a child is going in the wrong direction, is no longer needed, or its work is made obsolete by another child. The child is left in `cancelled` status; you can still send it a new message with `send_to_child`. Cancelling a child that is not running has no effect on that child.

If the child is itself an orchestrator, cancelling it also cancels the turns of its own running children, and theirs in turn. Cancelling a thread's own turn never cancels its children: they run in parallel by design. Children are cancelled when their orchestrator's thread is closed, archived or deleted.";

pub struct CancelChildTool {
    session_id: String,
}

impl CancelChildTool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds its orchestrator at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for CancelChildTool {
    type Input = CancelChildInput;
    type Output = OrchestrationToolOutput<CancelChildOutput>;

    const NAME: &'static str = "cancel_child";

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
        "Cancel child".into()
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
            |host, orchestrator, input, cx| host.cancel_child(orchestrator, input, cx),
        )
    }
}
