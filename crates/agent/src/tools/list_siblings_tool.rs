use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{ListSiblingsInput, ListSiblingsOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
List your siblings: the other child threads of the orchestrator that started you.

Returns, for each sibling, its `child_id`, title, agent, model and status. Use it to find who is working next to you before you ask something with `send_to_sibling`. It never returns what a sibling has said or done.

Only a thread that was started by an orchestrator has this tool.";

pub struct ListSiblingsTool {
    session_id: String,
}

impl ListSiblingsTool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds the calling thread at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for ListSiblingsTool {
    type Input = ListSiblingsInput;
    type Output = OrchestrationToolOutput<ListSiblingsOutput>;

    const NAME: &'static str = "list_siblings";

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
        "List sibling threads".into()
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
            |host, thread, _input, cx| Task::ready(host.list_siblings(thread, cx)),
        )
    }
}
