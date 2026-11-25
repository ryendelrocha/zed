use agent_client_protocol::v2::schema as acp;
use gpui::{App, SharedString, Task};
use orchestration::{StartChildInput, StartChildOutput};
use std::sync::Arc;

use super::orchestration_common::{OrchestrationToolOutput, run_orchestration_tool};
use crate::{AgentTool, ToolCallEventStream, ToolInput};

/// Tool description shown to the model.
const DESCRIPTION: &str = "\
Start a child thread that works in parallel with you, and return right away.

Use this to split a task into independent pieces that can run at the same time (for example, one child per module, per file group or per investigation). The child is a full agent thread: it appears in the threads sidebar under this thread and works in the same project.

How results come back:
- You do NOT see the child's transcript. When a child finishes its turn, you receive only its final message, through `await_children`.
- The child has no access to this conversation's history. Put everything it needs in `prompt`: the goal, relevant file paths, constraints, the expected shape of the final message, and what it must NOT touch.
- Children run in parallel, so give each child files or areas that do not overlap, or use `worktree: new` to isolate it.

Typical flow: call `start_child` once per piece of work (all in the same turn), then call `await_children` to collect the final messages. Prefer `await_children` over polling with `child_status`.

Agent, profile and model: leave `agent`, `profile` and `model` unset to use the user's current defaults. Call `list_agents_and_models` first when you need something specific.
- Prefer `profile` over naming a `model`. Profiles are listed by `list_agents_and_models`, each with a description of when to use it. Match the task to the descriptions and pick the best fit. The child gets the profile's tools and its default model. In your reply to the user, say which profile you used for each child and why.
- Use `model` only when no profile fits or you need an exact model. It wins over the profile's model. The model resolves in this order: `model`, the profile's default model, the user's subagent model setting, the user's default model.
- `profile` and `model` are applied only for the native Zed agent; for an external agent they are ignored and the result carries a warning. An unknown `profile` is an error that lists the valid ids.

Worktree: `shared` (default) uses this thread's worktree. `new` creates a separate git worktree in a new workspace, based on `base_ref` (default HEAD), so the child cannot collide with you or with other children. It needs a git repository in the project.

At most `agent.orchestration.max_concurrent_children` children (default 6) may run at the same time across the whole app. Beyond that, `start_child` fails with an error that states how many are running: call `await_children` to let some finish, then start the next one. Children that finished do not count.

Children can message each other directly with `send_to_sibling`; you only see how many sibling messages each exchanged.

Returns the `child_id` to use with the other orchestration tools, and the path of the log of the exchanges with the child.";

pub struct StartChildTool {
    session_id: String,
}

impl StartChildTool {
    /// `session_id` is the session of the thread that owns the tool. It is how
    /// the tool finds its orchestrator at run time.
    pub fn new(session_id: String) -> Self {
        Self { session_id }
    }
}

impl AgentTool for StartChildTool {
    type Input = StartChildInput;
    type Output = OrchestrationToolOutput<StartChildOutput>;

    const NAME: &'static str = "start_child";

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
            Ok(input) => format!("Start child: {}", input.title).into(),
            Err(value) => value
                .get("title")
                .and_then(|title| title.as_str())
                .map(|title| format!("Start child: {title}").into())
                .unwrap_or_else(|| "Start child".into()),
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
            |host, orchestrator, input, cx| host.start_child(orchestrator, input, cx),
        )
    }
}
