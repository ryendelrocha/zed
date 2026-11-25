use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{SendToChildInput, SendToChildOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
Send a follow-up message to a child thread you started.

Use it to correct a child, answer a question it asked in its final message or with `send_to_parent`, or hand it the next step. The child keeps the context of its earlier turns, so send a short, direct message and do not repeat the original task.

If the child is in the middle of a turn, or its agent is still connecting, the message is queued and delivered when it can take it (`queued` is true). If the child already finished, the message starts a new turn and its status goes back to `running`. If the child is not loaded (for example after Zed restarted), Zed opens it first, so the call still works.

The reply does not come back by itself: call `await_children` to get the child's next final message.";

pub struct SendToChildTool {
    session_id: String,
}

impl SendToChildTool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds its orchestrator at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for SendToChildTool {
    type Input = SendToChildInput;
    type Output = OrchestrationToolOutput<SendToChildOutput>;

    const NAME: &'static str = "send_to_child";

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
        "Send to child".into()
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
            |host, orchestrator, input, cx| host.send_to_child(orchestrator, input, cx),
        )
    }
}
