use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{SendToSiblingInput, SendToSiblingOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
Send a message straight to a sibling: another child of the orchestrator that started you. The orchestrator does not receive it.

Use it for a quick fact you need from a thread working next to you (\"which file are you changing?\", \"what did the API return?\"). Find the sibling's `child_id` with `list_siblings`. Do not use it to hand in your result or to report a blocker: your final message goes to the orchestrator automatically, and `send_to_parent` reaches it during the task.

The message is added to the sibling's thread as a new message prefixed \"[from sibling <title>]\", once the sibling is idle. It never interrupts a turn the sibling is running. Both of you have the exchange in your logs, and the orchestrator is told only how many sibling messages you exchanged.

With `await_reply: true` you wait, up to `timeout_secs` (default 120), for the sibling's answer. The answer is the next `send_to_sibling` the sibling sends to you: it is told to answer that way. If it does not answer in time, `timed_out` is true and the message was still delivered. `await_reply` fails with a deadlock error when the sibling is already waiting for a reply from you, directly or through other siblings: answer it with a plain `send_to_sibling` instead, which unblocks it.

You and each sibling can exchange at most `agent.orchestration.max_sibling_messages` messages (default 20, both directions). After that the call fails and you should go through the orchestrator with `send_to_parent`.

Only a thread that was started by an orchestrator has this tool.";

pub struct SendToSiblingTool {
    session_id: String,
}

impl SendToSiblingTool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds the calling thread at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for SendToSiblingTool {
    type Input = SendToSiblingInput;
    type Output = OrchestrationToolOutput<SendToSiblingOutput>;

    const NAME: &'static str = "send_to_sibling";

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
            Ok(input) if input.await_reply => "Ask sibling and wait for the reply".into(),
            _ => "Send message to sibling".into(),
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
            |host, thread, input, cx| host.send_to_sibling(thread, input, cx),
        )
    }
}
