use std::{cell::RefCell, rc::Rc};

use acp_thread::{AcpThread, AcpThreadEvent, ThreadStatus};
use agent_client_protocol::v2::schema as acp_v2;
use agent_settings::AgentSettings;
use anyhow::{Context as _, Result, anyhow, ensure};
use collections::HashMap;
use gpui::{App, Entity, Subscription, Task, WeakEntity};
use orchestration::{
    AvailableAgent, AvailableAgents, AvailableModel, CancelChildInput, CancelChildOutput,
    ChildStatus, NewChild, OrchestrationHost, OrchestrationRegistry, SendToChildInput,
    SendToChildOutput, StartChildInput, StartChildOutput, ThreadId, WorktreeMode,
    orchestration_log_dir,
};
use settings::Settings as _;

use crate::{
    AgentPanel,
    agent_panel::{ChildThreadRequest, create_child_thread},
    conversation_view::{ConversationView, RootThreadUpdated},
};

/// Serves the orchestration tools from an agent panel: it creates child
/// threads, delivers messages to them, and watches each child's `AcpThread`
/// to keep the [`OrchestrationRegistry`] and the child's log up to date.
///
/// Children may live in another panel (when started with `worktree: new`), so
/// the host keeps its own weak handle to each child's `ConversationView`.
pub(crate) struct AgentPanelOrchestrationHost {
    panel: WeakEntity<AgentPanel>,
    window: gpui::AnyWindowHandle,
    children: Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
}

struct ChildHandle {
    conversation_view: WeakEntity<ConversationView>,
    thread_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl AgentPanelOrchestrationHost {
    pub(crate) fn new(panel: WeakEntity<AgentPanel>, window: gpui::AnyWindowHandle) -> Self {
        Self {
            panel,
            window,
            children: Rc::default(),
        }
    }

    fn ensure_enabled(cx: &App) -> Result<()> {
        ensure!(
            AgentSettings::get_global(cx).orchestration.enabled,
            "Orchestration is turned off by the `agent.orchestration.enabled` setting."
        );
        Ok(())
    }

    fn child_view(&self, child: ThreadId) -> Result<Entity<ConversationView>> {
        self.children
            .borrow()
            .get(&child)
            .and_then(|handle| handle.conversation_view.upgrade())
            .context("The child thread is no longer open, so it cannot be reached.")
    }
}

impl OrchestrationHost for AgentPanelOrchestrationHost {
    fn start_child(
        &self,
        orchestrator: ThreadId,
        input: StartChildInput,
        cx: &mut App,
    ) -> Task<Result<StartChildOutput>> {
        if let Err(error) = Self::ensure_enabled(cx) {
            return Task::ready(Err(error));
        }
        let panel = self.panel.clone();
        let window = self.window;
        let children = self.children.clone();
        cx.spawn(async move |cx| {
            let (owns_orchestrator, selected_agent_is_native) = panel
                .read_with(cx, |panel, cx| {
                    (
                        panel.conversation_view_for_id(&orchestrator, cx).is_some(),
                        panel.selected_agent_is_native(),
                    )
                })
                .context("The agent panel is no longer available.")?;
            ensure!(
                owns_orchestrator,
                "This thread is not an orchestrator that this agent panel owns."
            );

            let is_native = match input.agent.as_deref() {
                Some(agent_id) => agent_id == agent::ZED_AGENT_ID.as_ref(),
                None => selected_agent_is_native,
            };
            let mut warnings = Vec::new();
            let model = match input.model {
                Some(model) if !is_native => {
                    warnings.push(format!(
                        "`model` ({model:?}) is not supported yet for external agents: \
                         the child uses the agent's own default model."
                    ));
                    None
                }
                Some(model)
                    if !model
                        .split_once('/')
                        .is_some_and(|(provider, id)| !provider.is_empty() && !id.is_empty()) =>
                {
                    warnings.push(format!(
                        "`model` ({model:?}) must be `provider/model-id`: \
                         the child uses the default model."
                    ));
                    None
                }
                model => model,
            };

            let child = create_child_thread(
                panel,
                window,
                ChildThreadRequest {
                    title: input.title.clone().into(),
                    prompt: input.prompt.clone(),
                    agent_id: input.agent,
                    model: model.clone(),
                    use_new_worktree: input.worktree == WorktreeMode::New,
                    base_ref: input.base_ref,
                    worktree_name: input.worktree_name,
                    parent_thread_id: Some(orchestrator),
                },
                cx,
            )
            .await?;
            warnings.extend(child.warning);

            let agent_id = child.agent_id.0.to_string();
            let registry = cx.update(OrchestrationRegistry::global_or_init);
            let log_dir = cx.update(|cx| orchestration_log_dir(cx));
            let log_path = registry.update(cx, |registry, cx| {
                registry.register_child(
                    orchestrator,
                    NewChild {
                        child_id: child.thread_id,
                        title: input.title.into(),
                        agent_id: agent_id.clone(),
                        model,
                        log_dir,
                        prompt: input.prompt,
                    },
                    cx,
                )
            });
            cx.update(|cx| {
                observe_child(
                    &children,
                    orchestrator,
                    child.thread_id,
                    &child.conversation_view,
                    cx,
                );
            });

            Ok(StartChildOutput {
                child_id: child.thread_id.to_key_string(),
                log_path: log_path.display().to_string(),
                agent: agent_id,
                warning: (!warnings.is_empty()).then(|| warnings.join(" ")),
            })
        })
    }

    fn send_to_child(
        &self,
        orchestrator: ThreadId,
        input: SendToChildInput,
        cx: &mut App,
    ) -> Task<Result<SendToChildOutput>> {
        let window = self.window;
        let conversation_view = (|| {
            Self::ensure_enabled(cx)?;
            let child = OrchestrationRegistry::global_or_init(cx)
                .read(cx)
                .resolve_child_id(orchestrator, &input.child_id)?;
            Ok((child, self.child_view(child)?))
        })();
        let (child, conversation_view) = match conversation_view {
            Ok(resolved) => resolved,
            Err(error) => return Task::ready(Err(error)),
        };
        // Deferred, because sending needs the window and the caller may be
        // running inside an update of it.
        cx.spawn(async move |cx| {
            let content = vec![acp_v2::ContentBlock::Text(acp_v2::TextContent::new(
                input.message.clone(),
            ))];
            let queued = window.update(cx, |_root, window, cx| {
                conversation_view.update(cx, |view, cx| view.send_follow_up(content, window, cx))
            })??;
            let registry = cx.update(OrchestrationRegistry::global_or_init);
            registry.update(cx, |registry, cx| {
                registry.record_message_to_child(orchestrator, child, &input.message, cx);
            });
            Ok(SendToChildOutput { queued })
        })
    }

    fn cancel_child(
        &self,
        orchestrator: ThreadId,
        input: CancelChildInput,
        cx: &mut App,
    ) -> Task<Result<CancelChildOutput>> {
        Task::ready((|| {
            Self::ensure_enabled(cx)?;
            let registry = OrchestrationRegistry::global_or_init(cx);
            let child = registry
                .read(cx)
                .resolve_child_id(orchestrator, &input.child_id)?;
            let status = registry
                .read(cx)
                .child(orchestrator, child)
                .map(|record| record.status)
                .context("Unknown child")?;
            if status.is_terminal() {
                return Ok(CancelChildOutput { status });
            }
            let conversation_view = self.child_view(child)?;
            let thread = conversation_view
                .read(cx)
                .root_thread(cx)
                .context("The child is still starting and cannot be cancelled yet.")?;
            let final_message = thread.read(cx).last_assistant_message(cx);
            conversation_view.update(cx, |view, cx| view.cancel_generation(cx));
            registry.update(cx, |registry, cx| {
                registry.record_turn_finished(
                    orchestrator,
                    child,
                    ChildStatus::Cancelled,
                    final_message,
                    cx,
                );
            });
            Ok(CancelChildOutput {
                status: ChildStatus::Cancelled,
            })
        })())
    }

    fn list_available_agents(&self, cx: &mut App) -> Result<AvailableAgents> {
        let panel = self
            .panel
            .upgrade()
            .ok_or_else(|| anyhow!("Agent panel is no longer available"))?;
        let available = panel.read(cx).available_agents(cx);
        Ok(AvailableAgents {
            agents: available
                .agents
                .into_iter()
                .map(|agent| AvailableAgent {
                    id: agent.id,
                    name: agent.name,
                    is_native: agent.is_native,
                    models: agent
                        .models
                        .into_iter()
                        .map(|model| AvailableModel {
                            id: model.id,
                            name: model.name,
                            is_default: model.is_default,
                        })
                        .collect(),
                })
                .collect(),
        })
    }

    fn resolve_orchestrator(&self, session_id: &str, cx: &App) -> Option<ThreadId> {
        let panel = self.panel.upgrade()?;
        panel
            .read(cx)
            .conversation_views()
            .into_iter()
            .find_map(|view| {
                let view = view.read(cx);
                let thread = view.root_thread(cx)?;
                (thread.read(cx).session_id().0.as_ref() == session_id).then_some(view.thread_id)
            })
    }

    fn owns_thread(&self, thread_id: ThreadId, cx: &App) -> bool {
        self.panel.upgrade().is_some_and(|panel| {
            panel
                .read(cx)
                .conversation_view_for_id(&thread_id, cx)
                .is_some()
        })
    }
}

/// Starts watching a child: its `ConversationView` for the moment its root
/// thread exists (or the agent fails to load), and then the `AcpThread`.
fn observe_child(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    orchestrator: ThreadId,
    child: ThreadId,
    conversation_view: &Entity<ConversationView>,
    cx: &mut App,
) {
    let root_subscription = cx.subscribe(conversation_view, {
        let children = children.clone();
        move |conversation_view, _: &RootThreadUpdated, cx| {
            attach_thread_observer(&children, orchestrator, child, &conversation_view, cx);
        }
    });
    let load_subscription = cx.observe(conversation_view, move |conversation_view, cx| {
        let Some(message) = conversation_view.read(cx).load_error_message() else {
            return;
        };
        finish_turn(
            orchestrator,
            child,
            ChildStatus::Failed,
            Some(format!("The agent failed to load: {message}")),
            cx,
        );
    });
    children.borrow_mut().insert(
        child,
        ChildHandle {
            conversation_view: conversation_view.downgrade(),
            thread_subscription: None,
            _subscriptions: vec![root_subscription, load_subscription],
        },
    );
    attach_thread_observer(children, orchestrator, child, conversation_view, cx);
}

fn attach_thread_observer(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    orchestrator: ThreadId,
    child: ThreadId,
    conversation_view: &Entity<ConversationView>,
    cx: &mut App,
) {
    let already_attached = children
        .borrow()
        .get(&child)
        .is_none_or(|handle| handle.thread_subscription.is_some());
    if already_attached {
        return;
    }
    let Some(thread) = conversation_view.read(cx).root_thread(cx) else {
        return;
    };
    let subscription = cx.subscribe(&thread, move |thread, event, cx| {
        on_thread_event(orchestrator, child, &thread, event, cx);
    });
    if let Some(handle) = children.borrow_mut().get_mut(&child) {
        handle.thread_subscription = Some(subscription);
    }
    sync_live_status(orchestrator, child, &thread, cx);
}

fn on_thread_event(
    orchestrator: ThreadId,
    child: ThreadId,
    thread: &Entity<AcpThread>,
    event: &AcpThreadEvent,
    cx: &mut App,
) {
    match event {
        AcpThreadEvent::Stopped { stop_reason, .. } => {
            let status = match stop_reason {
                Some(acp_v2::StopReason::Cancelled) => ChildStatus::Cancelled,
                Some(
                    acp_v2::StopReason::Refusal
                    | acp_v2::StopReason::MaxTokens
                    | acp_v2::StopReason::Error(_),
                ) => ChildStatus::Failed,
                _ => ChildStatus::Completed,
            };
            let message = thread.read(cx).last_assistant_message(cx);
            finish_turn(orchestrator, child, status, message, cx);
        }
        AcpThreadEvent::Error => {
            let message = failure_message(thread, "The child stopped with an error.", cx);
            finish_turn(orchestrator, child, ChildStatus::Failed, Some(message), cx);
        }
        AcpThreadEvent::ConnectionLost => {
            let message =
                failure_message(thread, "The child lost its connection to the agent.", cx);
            finish_turn(orchestrator, child, ChildStatus::Failed, Some(message), cx);
        }
        AcpThreadEvent::LoadError(error) => {
            finish_turn(
                orchestrator,
                child,
                ChildStatus::Failed,
                Some(format!("The agent failed to load: {error}")),
                cx,
            );
        }
        AcpThreadEvent::StatusChanged
        | AcpThreadEvent::ToolAuthorizationRequested(_)
        | AcpThreadEvent::ToolAuthorizationReceived(_)
        | AcpThreadEvent::ElicitationRequested(_)
        | AcpThreadEvent::ElicitationResponded(_) => {
            sync_live_status(orchestrator, child, thread, cx);
        }
        _ => {}
    }
}

/// `Refusal` is not handled on its own: the turn's `Stopped` event follows it.
fn failure_message(thread: &Entity<AcpThread>, fallback: &str, cx: &App) -> String {
    match thread.read(cx).last_assistant_message(cx) {
        Some(last) => format!("{fallback} Last message: {last}"),
        None => fallback.to_string(),
    }
}

/// Moves a running child between `Running` and `WaitingForConfirmation`.
fn sync_live_status(
    orchestrator: ThreadId,
    child: ThreadId,
    thread: &Entity<AcpThread>,
    cx: &mut App,
) {
    let thread = thread.read(cx);
    let status = if thread.is_waiting_for_confirmation() {
        ChildStatus::WaitingForConfirmation
    } else if thread.status() == ThreadStatus::Generating {
        ChildStatus::Running
    } else {
        return;
    };
    OrchestrationRegistry::global_or_init(cx).update(cx, |registry, cx| {
        registry.set_status(orchestrator, child, status, cx);
    });
}

/// Records the end of a turn once: several events can report the same end
/// (for example an error followed by a stop), and only the first counts.
fn finish_turn(
    orchestrator: ThreadId,
    child: ThreadId,
    status: ChildStatus,
    message: Option<String>,
    cx: &mut App,
) {
    OrchestrationRegistry::global_or_init(cx).update(cx, |registry, cx| {
        let already_finished = registry
            .child(orchestrator, child)
            .is_none_or(|record| record.status.is_terminal());
        if !already_finished {
            registry.record_turn_finished(orchestrator, child, status, message, cx);
        }
    });
}
