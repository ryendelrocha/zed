use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{WhoAmIInput, WhoAmIOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
Report this thread's place in an orchestration.

Returns your `role` (`orchestrator` if you started child threads, `child` if an orchestrator started you, `both`, or `none`), your `parent` (id and title of the orchestrator that started you, if any) and your `children` (id, title and status of each thread you started).

Call it when you are unsure whether someone started you. If you have a parent, reach it with `send_to_parent`, not in your own chat.";

pub struct WhoAmITool {
    session_id: String,
}

impl WhoAmITool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds the calling thread at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for WhoAmITool {
    type Input = WhoAmIInput;
    type Output = OrchestrationToolOutput<WhoAmIOutput>;

    const NAME: &'static str = "whoami";

    fn description() -> SharedString {
        DESCRIPTION.into()
    }

    fn kind() -> acp::ToolKind {
        acp::ToolKind::Other
    }

    fn initial_title(
        &self,
        _input: Result<Self::Input, serde_json::Value>,
        _cx: &mut App,
    ) -> SharedString {
        "Check role in orchestration".into()
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
            |host, thread, _input, cx| Task::ready(Ok(host.whoami(thread, cx))),
        )
    }
}
