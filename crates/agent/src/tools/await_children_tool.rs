use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{AwaitChildrenInput, AwaitChildrenOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
Wait until child threads finish their turn, and return only each child's final message.

This is the way to collect results from children started with `start_child`. Prefer it over polling `child_status`: it blocks until every requested child is `completed`, `failed` or `cancelled`, or until `timeout_secs` expires.

- `child_ids`: the children to wait for. Leave it empty to wait for every child of this thread.
- Children run in parallel, so waiting on several costs about as much as waiting on the slowest one.
- Each result has the child's status and, once it finished, its final message: the last message it wrote in its latest turn. You never get its transcript or its tool calls.
- If the timeout expires first, `timed_out` is true and the unfinished children come back with their current status and no message. Call `await_children` again to keep waiting; do other useful work in between if you have any.
- A `failed` child stopped with an error or a refusal; a `cancelled` child was cancelled. The final message, when present, says what happened.
- To ask a finished child for more, use `send_to_child`, then `await_children` again.";

pub struct AwaitChildrenTool {
    session_id: String,
}

impl AwaitChildrenTool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds its orchestrator at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for AwaitChildrenTool {
    type Input = AwaitChildrenInput;
    type Output = OrchestrationToolOutput<AwaitChildrenOutput>;

    const NAME: &'static str = "await_children";

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
        "Await children".into()
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
            |host, orchestrator, input, cx| host.await_children(orchestrator, input, cx),
        )
    }
}
