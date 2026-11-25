//! Shared plumbing of the orchestration tools (`start_child`,
//! `await_children`, `child_status`, `send_to_child`, `cancel_child`).
//!
//! Each tool is a thin adapter: it resolves the calling thread's session to
//! its orchestrator and host through [`OrchestrationRegistry`], calls the host
//! and returns the host's output.

use std::rc::Rc;

use anyhow::Result;
use gpui::{App, Task};
use language_model::LanguageModelToolResultContent;
use orchestration::{OrchestrationHost, OrchestrationRegistry, ThreadId};
use serde::{Deserialize, Serialize};

use crate::ToolInput;

/// Names of the tools that exist only while orchestration is available.
pub const ORCHESTRATION_TOOL_NAMES: [&str; 9] = [
    "start_child",
    "await_children",
    "child_status",
    "send_to_child",
    "cancel_child",
    "send_to_parent",
    "list_siblings",
    "send_to_sibling",
    "whoami",
];

/// Whether `tool_name` is one of the orchestration tools.
pub fn is_orchestration_tool(tool_name: &str) -> bool {
    ORCHESTRATION_TOOL_NAMES.contains(&tool_name)
}

/// Result of an orchestration tool: the host's output, or an error the model
/// can read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrchestrationToolOutput<T> {
    Success(T),
    Error { error: String },
}

impl<T: Serialize> From<OrchestrationToolOutput<T>> for LanguageModelToolResultContent {
    fn from(output: OrchestrationToolOutput<T>) -> Self {
        serde_json::to_string(&output)
            .unwrap_or_else(|error| format!("Failed to serialize orchestration output: {error}"))
            .into()
    }
}

impl<T> OrchestrationToolOutput<T> {
    fn error(error: impl ToString) -> Self {
        Self::Error {
            error: error.to_string(),
        }
    }
}

/// Finds the orchestrator thread and the host for the calling session.
pub fn resolve_orchestrator(
    session_id: &str,
    cx: &App,
) -> std::result::Result<(ThreadId, Rc<dyn OrchestrationHost>), String> {
    OrchestrationRegistry::try_global(cx)
        .and_then(|registry| registry.read(cx).host_for_session(session_id, cx))
        .ok_or_else(|| {
            "Orchestration is not available for this thread: no agent panel hosts it. \
             Orchestration works only in a thread of the agent panel, and requires \
             `agent.orchestration.enabled`."
                .to_string()
        })
}

/// Whether the calling session can use the orchestration tools right now.
pub fn orchestration_available(session_id: &str, cx: &App) -> bool {
    resolve_orchestrator(session_id, cx).is_ok()
}

/// Whether the calling session can use `tool_name`, one of
/// [`ORCHESTRATION_TOOL_NAMES`], right now. `send_to_parent`, `list_siblings`
/// and `send_to_sibling` also need the thread to have been started by an
/// orchestrator.
pub fn orchestration_tool_available(tool_name: &str, session_id: &str, cx: &App) -> bool {
    let Ok((thread, _)) = resolve_orchestrator(session_id, cx) else {
        return false;
    };
    if !matches!(
        tool_name,
        "send_to_parent" | "list_siblings" | "send_to_sibling"
    ) {
        return true;
    }
    OrchestrationRegistry::try_global(cx)
        .is_some_and(|registry| registry.read(cx).orchestrator_of(thread).is_some())
}

/// Receives the tool input, resolves the orchestrator and calls the host.
pub(crate) fn run_orchestration_tool<I, O>(
    session_id: String,
    input: ToolInput<I>,
    cx: &mut App,
    call: impl FnOnce(Rc<dyn OrchestrationHost>, ThreadId, I, &mut App) -> Task<Result<O>> + 'static,
) -> Task<std::result::Result<OrchestrationToolOutput<O>, OrchestrationToolOutput<O>>>
where
    I: serde::de::DeserializeOwned + 'static,
    O: 'static,
{
    cx.spawn(async move |cx| {
        let input = input.recv().await.map_err(|error| {
            OrchestrationToolOutput::error(format!("Failed to receive tool input: {error}"))
        })?;
        let (orchestrator, host) = cx
            .update(|cx| resolve_orchestrator(&session_id, cx))
            .map_err(OrchestrationToolOutput::error)?;
        let task = cx.update(|cx| call(host, orchestrator, input, cx));
        match task.await {
            Ok(output) => Ok(OrchestrationToolOutput::Success(output)),
            Err(error) => Err(OrchestrationToolOutput::error(error)),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentTool, AwaitChildrenTool, CancelChildTool, ChildStatusTool, ListSiblingsTool,
        SendToChildTool, SendToParentTool, SendToSiblingTool, StartChildTool, ToolCallEventStream,
        WhoAmITool,
    };
    use gpui::TestAppContext;
    use orchestration::{
        AvailableAgents, AwaitChildrenInput, AwaitChildrenOutput, CancelChildInput,
        CancelChildOutput, ChildResult, ChildStatus, ChildStatusInput, ChildStatusOutput,
        SendToChildInput, SendToChildOutput, SendToParentInput, SendToParentOutput,
        StartChildInput, StartChildOutput, ThreadReference, ThreadRole, WhoAmIOutput, WorktreeMode,
    };
    use std::{cell::RefCell, sync::Arc};

    const SESSION: &str = "session-1";

    /// Host that records the orchestrator it was called for and returns
    /// canned outputs.
    struct FakeHost {
        orchestrator: ThreadId,
        calls: RefCell<Vec<(&'static str, ThreadId)>>,
    }

    impl FakeHost {
        fn record(&self, name: &'static str, orchestrator: ThreadId) {
            self.calls.borrow_mut().push((name, orchestrator));
        }
    }

    impl OrchestrationHost for FakeHost {
        fn start_child(
            &self,
            orchestrator: ThreadId,
            input: StartChildInput,
            _cx: &mut App,
        ) -> Task<Result<StartChildOutput>> {
            self.record("start_child", orchestrator);
            if input.agent.as_deref() == Some("unknown") {
                return Task::ready(Err(anyhow::anyhow!("Unknown agent id \"unknown\"")));
            }
            Task::ready(Ok(StartChildOutput {
                child_id: "child-1".into(),
                log_path: format!("/logs/{}.jsonl", input.title),
                agent: "zed".into(),
                warning: None,
            }))
        }

        fn send_to_child(
            &self,
            orchestrator: ThreadId,
            input: SendToChildInput,
            _cx: &mut App,
        ) -> Task<Result<SendToChildOutput>> {
            self.record("send_to_child", orchestrator);
            Task::ready(Ok(SendToChildOutput {
                queued: input.message == "queue me",
            }))
        }

        fn cancel_child(
            &self,
            orchestrator: ThreadId,
            _input: CancelChildInput,
            _cx: &mut App,
        ) -> Task<Result<CancelChildOutput>> {
            self.record("cancel_child", orchestrator);
            Task::ready(Ok(CancelChildOutput {
                status: ChildStatus::Cancelled,
            }))
        }

        fn list_available_agents(&self, _cx: &mut App) -> Result<AvailableAgents> {
            Ok(AvailableAgents::default())
        }

        fn resolve_orchestrator(&self, session_id: &str, _cx: &App) -> Option<ThreadId> {
            (session_id == SESSION).then_some(self.orchestrator)
        }

        fn owns_thread(&self, thread_id: ThreadId, _cx: &App) -> bool {
            thread_id == self.orchestrator
        }

        fn child_status(
            &self,
            orchestrator: ThreadId,
            _input: ChildStatusInput,
            _cx: &mut App,
        ) -> Task<Result<ChildStatusOutput>> {
            self.record("child_status", orchestrator);
            Task::ready(Ok(ChildStatusOutput { children: vec![] }))
        }

        fn send_to_parent(
            &self,
            caller: ThreadId,
            input: SendToParentInput,
            _cx: &mut App,
        ) -> Task<Result<SendToParentOutput>> {
            self.record("send_to_parent", caller);
            Task::ready(Ok(SendToParentOutput {
                parent_id: "parent-1".into(),
                parent_title: "Parent".into(),
                urgent: input.urgent,
            }))
        }

        fn list_siblings(
            &self,
            caller: ThreadId,
            _cx: &mut App,
        ) -> Result<orchestration::ListSiblingsOutput> {
            self.record("list_siblings", caller);
            Ok(orchestration::ListSiblingsOutput {
                siblings: vec![orchestration::SiblingInfo {
                    child_id: "sibling-1".into(),
                    title: "Sibling".into(),
                    agent: "zed".into(),
                    model: None,
                    status: ChildStatus::Running,
                }],
            })
        }

        fn send_to_sibling(
            &self,
            caller: ThreadId,
            input: orchestration::SendToSiblingInput,
            _cx: &mut App,
        ) -> Task<Result<orchestration::SendToSiblingOutput>> {
            self.record("send_to_sibling", caller);
            Task::ready(Ok(orchestration::SendToSiblingOutput {
                child_id: input.child_id,
                title: "Sibling".into(),
                reply: input.await_reply.then(|| "the answer".to_string()),
                timed_out: false,
                messages_left: 19,
            }))
        }

        fn whoami(&self, caller: ThreadId, _cx: &mut App) -> WhoAmIOutput {
            self.record("whoami", caller);
            WhoAmIOutput {
                role: ThreadRole::Child,
                parent: Some(ThreadReference {
                    id: "parent-1".into(),
                    title: "Parent".into(),
                }),
                children: Vec::new(),
            }
        }

        fn await_children(
            &self,
            orchestrator: ThreadId,
            _input: AwaitChildrenInput,
            _cx: &mut App,
        ) -> Task<Result<AwaitChildrenOutput>> {
            self.record("await_children", orchestrator);
            Task::ready(Ok(AwaitChildrenOutput {
                children: vec![ChildResult {
                    child_id: "child-1".into(),
                    title: "Child".into(),
                    status: ChildStatus::Completed,
                    final_message: Some("done".into()),
                    pending_messages: Vec::new(),
                    human_interventions: Vec::new(),
                }],
                timed_out: false,
            }))
        }
    }

    fn register_host(cx: &mut TestAppContext) -> Rc<FakeHost> {
        let host = Rc::new(FakeHost {
            orchestrator: ThreadId::new(),
            calls: RefCell::new(Vec::new()),
        });
        cx.update(|cx| {
            let registry = OrchestrationRegistry::global_or_init(cx);
            registry.update(cx, |registry, _| registry.register_host(host.clone()));
        });
        host
    }

    fn start_input(title: &str, agent: Option<&str>) -> StartChildInput {
        StartChildInput {
            title: title.into(),
            prompt: "do it".into(),
            agent: agent.map(Into::into),
            model: None,
            profile: None,
            worktree: WorktreeMode::Shared,
            base_ref: None,
            worktree_name: None,
            subagent_quota: 0,
        }
    }

    fn run_tool<T: AgentTool>(
        tool: T,
        input: T::Input,
        cx: &mut TestAppContext,
    ) -> Task<Result<T::Output, T::Output>> {
        let (event_stream, _events) = ToolCallEventStream::test();
        cx.update(|cx| Arc::new(tool).run(ToolInput::resolved(input), event_stream, cx))
    }

    #[gpui::test]
    async fn test_start_child_is_refused_once_the_callers_subagent_quota_is_used(
        cx: &mut TestAppContext,
    ) {
        let host = register_host(cx);
        cx.update(|cx| {
            OrchestrationRegistry::global_or_init(cx).update(cx, |registry, _| {
                registry.set_subagent_quota(host.orchestrator, 1, "/unused".into());
            });
        });

        run_tool(
            StartChildTool::new(SESSION.to_string()),
            start_input("A", None),
            cx,
        )
        .await
        .ok()
        .expect("the first child is within the quota");
        let refused = run_tool(
            StartChildTool::new(SESSION.to_string()),
            start_input("B", None),
            cx,
        )
        .await
        .err()
        .expect("the second child is over the quota");
        let OrchestrationToolOutput::Error { error } = refused else {
            panic!("expected an error");
        };
        assert!(error.contains("Subagent quota exhausted"), "{error}");
        assert_eq!(
            host.calls.borrow().len(),
            1,
            "the refused call never reaches the host"
        );

        // A failed start gives its spawn back.
        cx.update(|cx| {
            OrchestrationRegistry::global_or_init(cx).update(cx, |registry, _| {
                registry.set_subagent_quota(host.orchestrator, 1, "/unused".into());
            });
        });
        run_tool(
            StartChildTool::new(SESSION.to_string()),
            start_input("C", Some("unknown")),
            cx,
        )
        .await
        .err()
        .expect("an unknown agent fails");
        run_tool(
            StartChildTool::new(SESSION.to_string()),
            start_input("D", None),
            cx,
        )
        .await
        .ok()
        .expect("the failed start did not use the quota");
    }

    #[gpui::test]
    async fn test_each_tool_calls_the_host_for_its_orchestrator(cx: &mut TestAppContext) {
        let host = register_host(cx);
        let session = || SESSION.to_string();

        let started = run_tool(StartChildTool::new(session()), start_input("A", None), cx)
            .await
            .ok()
            .unwrap();
        let OrchestrationToolOutput::Success(started) = started else {
            panic!("expected success");
        };
        assert_eq!(started.child_id, "child-1");
        assert_eq!(started.log_path, "/logs/A.jsonl");

        let awaited = run_tool(
            AwaitChildrenTool::new(session()),
            AwaitChildrenInput::default(),
            cx,
        )
        .await
        .ok()
        .unwrap();
        let OrchestrationToolOutput::Success(awaited) = awaited else {
            panic!("expected success");
        };
        assert_eq!(awaited.children[0].final_message.as_deref(), Some("done"));

        let status = run_tool(
            ChildStatusTool::new(session()),
            ChildStatusInput::default(),
            cx,
        )
        .await
        .ok()
        .unwrap();
        assert!(matches!(status, OrchestrationToolOutput::Success(_)));

        let sent = run_tool(
            SendToChildTool::new(session()),
            SendToChildInput {
                child_id: "child-1".into(),
                message: "queue me".into(),
            },
            cx,
        )
        .await
        .ok()
        .unwrap();
        let OrchestrationToolOutput::Success(sent) = sent else {
            panic!("expected success");
        };
        assert!(sent.queued);

        let cancelled = run_tool(
            CancelChildTool::new(session()),
            CancelChildInput {
                child_id: "child-1".into(),
            },
            cx,
        )
        .await
        .ok()
        .unwrap();
        let OrchestrationToolOutput::Success(cancelled) = cancelled else {
            panic!("expected success");
        };
        assert_eq!(cancelled.status, ChildStatus::Cancelled);

        let calls = host.calls.borrow();
        let names: Vec<_> = calls.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "start_child",
                "await_children",
                "child_status",
                "send_to_child",
                "cancel_child"
            ]
        );
        assert!(calls.iter().all(|(_, thread)| *thread == host.orchestrator));
    }

    #[gpui::test]
    async fn test_child_tools_call_the_host_for_the_calling_thread(cx: &mut TestAppContext) {
        let host = register_host(cx);

        let sent = run_tool(
            SendToParentTool::new(SESSION.into()),
            SendToParentInput {
                message: "I am blocked".into(),
                urgent: true,
            },
            cx,
        )
        .await
        .ok()
        .unwrap();
        let OrchestrationToolOutput::Success(sent) = sent else {
            panic!("expected success");
        };
        assert!(sent.urgent);
        assert_eq!(sent.parent_title, "Parent");

        let who = run_tool(
            WhoAmITool::new(SESSION.into()),
            orchestration::WhoAmIInput {},
            cx,
        )
        .await
        .ok()
        .unwrap();
        let OrchestrationToolOutput::Success(who) = who else {
            panic!("expected success");
        };
        assert_eq!(who.role, ThreadRole::Child);

        let calls = host.calls.borrow();
        let names: Vec<_> = calls.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["send_to_parent", "whoami"]);
        assert!(calls.iter().all(|(_, thread)| *thread == host.orchestrator));
    }

    #[gpui::test]
    async fn test_sibling_tools_call_the_host_for_the_calling_thread(cx: &mut TestAppContext) {
        let host = register_host(cx);

        let listed = run_tool(
            ListSiblingsTool::new(SESSION.into()),
            orchestration::ListSiblingsInput {},
            cx,
        )
        .await
        .ok()
        .unwrap();
        let OrchestrationToolOutput::Success(listed) = listed else {
            panic!("expected success");
        };
        assert_eq!(listed.siblings[0].child_id, "sibling-1");

        let sent = run_tool(
            SendToSiblingTool::new(SESSION.into()),
            orchestration::SendToSiblingInput {
                child_id: "sibling-1".into(),
                message: "which file?".into(),
                await_reply: true,
                timeout_secs: None,
            },
            cx,
        )
        .await
        .ok()
        .unwrap();
        let OrchestrationToolOutput::Success(sent) = sent else {
            panic!("expected success");
        };
        assert_eq!(sent.reply.as_deref(), Some("the answer"));

        let calls = host.calls.borrow();
        let names: Vec<_> = calls.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["list_siblings", "send_to_sibling"]);
        assert!(calls.iter().all(|(_, thread)| *thread == host.orchestrator));
    }

    #[gpui::test]
    async fn test_host_errors_reach_the_model(cx: &mut TestAppContext) {
        register_host(cx);
        let result = run_tool(
            StartChildTool::new(SESSION.into()),
            start_input("A", Some("unknown")),
            cx,
        )
        .await;
        let Err(OrchestrationToolOutput::Error { error }) = result else {
            panic!("expected an error");
        };
        assert!(error.contains("Unknown agent id"), "{error}");
    }

    #[gpui::test]
    async fn test_thread_without_host_gets_a_clear_error(cx: &mut TestAppContext) {
        let host = register_host(cx);

        // A session no host owns, with a host registered.
        let result = run_tool(
            AwaitChildrenTool::new("other-session".into()),
            AwaitChildrenInput::default(),
            cx,
        )
        .await;
        let Err(OrchestrationToolOutput::Error { error }) = result else {
            panic!("expected an error");
        };
        assert!(error.contains("Orchestration is not available"), "{error}");
        assert!(host.calls.borrow().is_empty());
        assert!(!cx.update(|cx| orchestration_available("other-session", cx)));
        assert!(cx.update(|cx| orchestration_available(SESSION, cx)));
    }

    #[gpui::test]
    async fn test_no_registry_gets_a_clear_error(cx: &mut TestAppContext) {
        let result = run_tool(
            StartChildTool::new(SESSION.into()),
            start_input("A", None),
            cx,
        )
        .await;
        assert!(matches!(result, Err(OrchestrationToolOutput::Error { .. })));
    }

    #[test]
    fn test_tool_names_match_the_orchestration_list() {
        let names = [
            StartChildTool::NAME,
            AwaitChildrenTool::NAME,
            ChildStatusTool::NAME,
            SendToChildTool::NAME,
            CancelChildTool::NAME,
            SendToParentTool::NAME,
            ListSiblingsTool::NAME,
            SendToSiblingTool::NAME,
            WhoAmITool::NAME,
        ];
        assert_eq!(names, ORCHESTRATION_TOOL_NAMES);
        for name in names {
            assert!(is_orchestration_tool(name));
        }
        assert!(!is_orchestration_tool("create_thread"));
    }
}
