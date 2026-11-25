use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{SendToParentInput, SendToParentOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
Send a message to the orchestrator that started this thread.

Use this, and not your own chat, to reach your orchestrator during the task: a blocker, a question, a decision you need, something it should know. The orchestrator cannot read your chat. You do not need it to hand in your result: your final message goes back to the orchestrator automatically when you finish.

The message is kept in the orchestrator's inbox and it sees it the next time it checks its children. Set `urgent: true` only when you are blocked and cannot continue without an answer: the message is then also added to the orchestrator's thread as a new message, once the orchestrator is idle (it is never interrupted in the middle of a turn). The answer comes back to you as a new message in this thread.

Only a thread that was started by an orchestrator has this tool. Call `whoami` to see your orchestrator.";

pub struct SendToParentTool {
    session_id: String,
}

impl SendToParentTool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds the calling thread at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for SendToParentTool {
    type Input = SendToParentInput;
    type Output = OrchestrationToolOutput<SendToParentOutput>;

    const NAME: &'static str = "send_to_parent";

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
        match input {
            Ok(input) if input.urgent => "Send urgent message to orchestrator".into(),
            _ => "Send message to orchestrator".into(),
        }
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
            |host, thread, input, cx| host.send_to_parent(thread, input, cx),
        )
    }
}
