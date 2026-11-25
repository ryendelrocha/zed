use std::{cell::RefCell, rc::Rc};

use acp_thread::{
    AcpThread, AcpThreadEvent, AgentThreadEntry, AssistantMessageChunk, ThreadStatus,
};
use agent_client_protocol::v2::schema as acp_v2;
use agent_settings::AgentSettings;
use anyhow::{Context as _, Result, anyhow, ensure};
use collections::HashMap;
use futures::StreamExt as _;
use gpui::{App, Entity, Global, Subscription, Task, TaskExt as _, WeakEntity};
use language_model::{
    CompletionIntent, LanguageModelCompletionEvent, LanguageModelRegistry, LanguageModelRequest,
    LanguageModelRequestMessage, Role,
};
use orchestration::{
    AvailableAgent, AvailableAgents, AvailableModel, CancelChildInput, CancelChildOutput, ChildLog,
    ChildStatus, HumanSummaryRequest, MessageSender, NewChild, OrchestrationHost,
    OrchestrationRegistry, RestoredChild, SendToChildInput, SendToChildOutput, SenderRole,
    StartChildInput, StartChildOutput, ThreadId, WorktreeMode, child_header, human_summary_prompt,
    max_concurrent_children, orchestration_log_dir, resolve_child_model, unknown_profile_error,
};
use settings::Settings as _;

use crate::{
    AgentPanel,
    agent_panel::{ChildSubagentQuota, ChildThreadRequest, create_child_thread},
    conversation_view::{ConversationView, MessageOrigin, RootThreadUpdated},
    orchestration_models::{self, ModelGateOutcome},
    thread_metadata_store::ThreadMetadataStore,
};

struct SavedChildrenWatch;

impl gpui::Global for SavedChildrenWatch {}

/// Keeps the orchestration registry in line with the saved threads: every
/// thread that has a parent link and that nobody loaded gets a record under
/// its orchestrator, so `child_status`, `await_children` and `whoami` know the
/// children that existed before a restart. Safe to call from every panel: it
/// installs one observer for the app.
pub(crate) fn watch_saved_children(cx: &mut App) {
    orchestration_models::init(cx);
    if cx.has_global::<SavedChildrenWatch>() {
        return;
    }
    let Some(store) = ThreadMetadataStore::try_global(cx) else {
        return;
    };
    cx.set_global(SavedChildrenWatch);
    // Deferred because the store also notifies while a panel is being created
    // or updated, and the restore reads panels through their hosts.
    cx.defer(restore_saved_children);
    cx.observe(&store, |_store, cx| cx.defer(restore_saved_children))
        .detach();
    // Quit observers run before the windows and their threads are torn down,
    // and teardown cancels the running turns. Recording the interruption first
    // keeps that cancellation from being logged as a `cancel_child`.
    cx.on_app_quit(|cx| {
        if let Some(registry) = OrchestrationRegistry::try_global(cx) {
            registry.update(cx, |registry, cx| {
                registry.interrupt_running_children(cx);
            });
        }
        async {}
    })
    .detach();
}

pub(crate) fn restore_saved_children(cx: &mut App) {
    if !AgentSettings::get_global(cx).orchestration.enabled {
        return;
    }
    let Some(store) = ThreadMetadataStore::try_global(cx) else {
        return;
    };
    let registry = OrchestrationRegistry::global_or_init(cx);
    let log_dir = orchestration_log_dir(cx);
    let (restored, held) = {
        let store = store.read(cx);
        let registry = registry.read(cx);
        let mut held = Vec::new();
        let restored = store
            .entries()
            .filter(|metadata| !metadata.archived)
            .filter_map(|metadata| {
                let parent = store.entry(metadata.parent_thread_id?)?;
                if registry
                    .child(parent.thread_id, metadata.thread_id)
                    .is_some()
                {
                    return None;
                }
                // A panel that holds the conversation is either starting the
                // child (its log does not exist until `register_child` writes
                // the first prompt) or restored it at startup, and in that
                // case nothing else registers it.
                let holds_conversation = registry.host_for_thread(metadata.thread_id, cx).is_some();
                if holds_conversation {
                    let log = ChildLog::new(&log_dir, parent.thread_id, metadata.thread_id);
                    if !log.path().is_file() {
                        return None;
                    }
                    held.push((parent.thread_id, metadata.thread_id));
                }
                Some((
                    parent.thread_id,
                    RestoredChild {
                        child_id: metadata.thread_id,
                        title: metadata.display_title(),
                        agent_id: metadata.agent_id.0.to_string(),
                        model: metadata
                            .run_config
                            .model()
                            .map(|(provider, model)| format!("{provider}/{model}")),
                        log_dir: log_dir.clone(),
                        orchestrator_title: parent.display_title().to_string(),
                    },
                ))
            })
            .collect::<Vec<_>>();
        (restored, held)
    };
    if restored.is_empty() {
        return;
    }
    registry.update(cx, |registry, cx| {
        for (orchestrator, child) in restored {
            registry.restore_child(orchestrator, child, cx);
        }
    });
    // The restored record is only a skeleton: the host that holds the
    // conversation has to observe it, or later turns would never update it.
    for (orchestrator, child) in held {
        let host = registry.read(cx).host_for_thread(child, cx);
        if let Some(host) = host {
            host.load_child(orchestrator, child, cx)
                .detach_and_log_err(cx);
        }
    }
}

/// Reports to the orchestration registry that a message was sent in `thread`.
/// A human message in a child counts toward its next summary. The message
/// itself is not stored anywhere.
pub(crate) fn note_turn(thread: ThreadId, origin: MessageOrigin, first_entry: usize, cx: &mut App) {
    let Some(registry) = OrchestrationRegistry::try_global(cx) else {
        return;
    };
    registry.update(cx, |registry, _| match origin {
        MessageOrigin::Human => registry.note_human_turn(thread, first_entry),
        MessageOrigin::Orchestration(_) => registry.note_orchestrator_turn(thread, first_entry),
    });
}

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
    /// Watches each orchestrator thread, to deliver urgent child messages
    /// when it stops.
    orchestrator_watches: Rc<RefCell<HashMap<ThreadId, Subscription>>>,
}

/// Every orchestrated child's conversation, whichever panel holds it. An
/// orchestrator's thread view uses it to show the children's permission
/// requests (R13) without going through a panel: children started with
/// `worktree: new` live in another workspace.
#[derive(Default)]
struct ChildConversations(HashMap<ThreadId, WeakEntity<ConversationView>>);

impl Global for ChildConversations {}

/// The conversation of an orchestrated child, while it is still open.
pub(crate) fn child_conversation_view(
    child: ThreadId,
    cx: &App,
) -> Option<Entity<ConversationView>> {
    cx.try_global::<ChildConversations>()?
        .0
        .get(&child)?
        .upgrade()
}

struct ChildHandle {
    conversation_view: WeakEntity<ConversationView>,
    thread_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    /// Messages for a child whose agent is still connecting: `send_to_child`
    /// does not fail, it parks the text here until the child's thread exists.
    /// While `model_gate` is set they also wait for the child's model.
    pending: Vec<String>,
    /// The orchestrator that wrote the parked messages.
    orchestrator: ThreadId,
    /// The model an external child must run on before its first prompt.
    model_gate: Option<ModelGate>,
}

/// The wait of an external child's first prompt for its model (R2). The
/// prompt is the first of `ChildHandle::pending`.
struct ModelGate {
    agent_id: String,
    requested: String,
    started: bool,
}

impl AgentPanelOrchestrationHost {
    pub(crate) fn new(panel: WeakEntity<AgentPanel>, window: gpui::AnyWindowHandle) -> Self {
        Self {
            panel,
            window,
            children: Rc::default(),
            orchestrator_watches: Rc::default(),
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

    /// The view of a child, opening the thread first when it is not loaded:
    /// after a restart, or after Zed unloaded an idle thread. The agent
    /// connects in the background, so the view may not be ready to receive
    /// messages yet: use [`send_or_queue`].
    fn open_child(
        &self,
        orchestrator: ThreadId,
        child: ThreadId,
        cx: &mut App,
    ) -> Task<Result<Entity<ConversationView>>> {
        if let Ok(view) = self.child_view(child) {
            return Task::ready(Ok(view));
        }
        let owned_elsewhere = OrchestrationRegistry::global_or_init(cx)
            .read(cx)
            .host_for_thread(child, cx)
            .is_some()
            && !self.owns_thread(child, cx);
        if owned_elsewhere {
            return Task::ready(Err(anyhow!(
                "The child is open in another workspace's agent panel, which this thread cannot \
                 reach. Open the child from the threads sidebar and try again."
            )));
        }
        let panel = self.panel.clone();
        let window = self.window;
        let children = self.children.clone();
        cx.spawn(async move |cx| {
            let view = window.update(cx, |_root, window, cx| {
                panel.update(cx, |panel, cx| {
                    panel.load_thread_in_background(child, window, cx)
                })
            })???;
            cx.update(|cx| observe_child(&children, window, orchestrator, child, &view, cx));
            Ok(view)
        })
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
        let limit = max_concurrent_children(cx);
        let registry = OrchestrationRegistry::global_or_init(cx);
        if let Err(error) = registry.update(cx, |registry, _| registry.reserve_child_slot(limit)) {
            return Task::ready(Err(error));
        }
        let panel = self.panel.clone();
        let window = self.window;
        let children = self.children.clone();
        let orchestrator_watches = self.orchestrator_watches.clone();
        let slot_registry = registry.downgrade();
        cx.spawn(async move |cx| {
            let result: Result<StartChildOutput> = async {
                let title = input.resolved_title();
                let (
                    orchestrator_view,
                    selected_agent_is_native,
                    selected_agent_id,
                    orchestrator_title,
                ) = panel
                    .read_with(cx, |panel, cx| {
                        let view = panel.conversation_view_for_id(&orchestrator, cx).cloned();
                        let title = view
                            .as_ref()
                            .and_then(|view| view.read(cx).root_thread(cx))
                            .and_then(|thread| thread.read(cx).title())
                            .map(|title| title.to_string())
                            .unwrap_or_else(|| "Untitled thread".to_string());
                        (
                            view,
                            panel.selected_agent_is_native(),
                            panel.selected_agent_id(),
                            title,
                        )
                    })
                    .context("The agent panel is no longer available.")?;
                let orchestrator_view = orchestrator_view
                    .context("This thread is not an orchestrator that this agent panel owns.")?;

                let is_native = match input.agent.as_deref() {
                    Some(agent_id) => agent_id == agent::ZED_AGENT_ID.as_ref(),
                    None => selected_agent_is_native,
                };
                let external_agent_id = (!is_native).then(|| {
                    input
                        .agent
                        .clone()
                        .unwrap_or_else(|| selected_agent_id.clone())
                });
                let mut warnings = Vec::new();

                // An unknown profile is an error whatever the agent, so a typo is
                // caught instead of being ignored with a warning.
                let (available_profiles, subagent_model) = cx.update(|cx| {
                    (
                        agent::available_profiles(cx),
                        agent_settings::AgentSettings::get_global(cx)
                            .subagent_model
                            .as_ref()
                            .map(agent::selection_to_model_string),
                    )
                });
                if let Some(profile) = input.profile.as_deref()
                    && !available_profiles.iter().any(|known| known.id == profile)
                {
                    return Err(anyhow!(unknown_profile_error(profile, &available_profiles)));
                }
                let profile_default_model = input.profile.as_deref().and_then(|profile| {
                    available_profiles
                        .iter()
                        .find(|known| known.id == profile)
                        .and_then(|known| known.default_model.clone())
                });
                // For an external agent a profile only matters through its
                // default model, and only when that model belongs to the agent
                // (`agent-id/model`).
                let profile_external_model = external_agent_id.as_deref().and_then(|agent_id| {
                    profile_default_model
                        .as_deref()
                        .and_then(|model| model.strip_prefix(agent_id)?.strip_prefix('/'))
                        .filter(|model| !model.is_empty())
                        .map(str::to_string)
                });
                let profile = match input.profile {
                    Some(profile) if !is_native => {
                        warnings.push(match &profile_external_model {
                            Some(model) => format!(
                                "`profile` ({profile:?}) is not supported yet for external \
                                 agents: only its default model {model:?} is used, and the child \
                                 otherwise uses the agent's own configuration."
                            ),
                            None => format!(
                                "`profile` ({profile:?}) is not supported yet for external \
                                 agents: the child uses the agent's own configuration."
                            ),
                        });
                        None
                    }
                    profile => profile,
                };

                // The model of an external child: explicit `model`, else the
                // profile's own model for that agent. It is set through the
                // agent's model option once the child connects.
                let external_model = match &external_agent_id {
                    Some(agent_id) => input
                        .model
                        .as_deref()
                        .map(|model| orchestration_models::strip_agent_prefix(agent_id, model))
                        .map(str::to_string)
                        .or(profile_external_model)
                        .filter(|model| !model.trim().is_empty()),
                    None => None,
                };
                // When the agent's models are already known (an open session,
                // or an earlier child), a model it does not offer is refused
                // now, before any thread exists.
                let early_model = match (&external_agent_id, &external_model) {
                    (Some(agent_id), Some(requested)) => {
                        cx.update(|cx| match orchestration_models::cached(agent_id, cx) {
                            Some(option) => orchestration_models::resolve_model(
                                agent_id,
                                &option.models,
                                requested,
                            )
                            .map(|model| Some(model.value.clone()))
                            .map_err(|error| anyhow!(error)),
                            None => Ok(None),
                        })?
                    }
                    _ => None,
                };

                let model = match input.model {
                    Some(_) if !is_native => None,
                    Some(model)
                        if !model.split_once('/').is_some_and(|(provider, id)| {
                            !provider.is_empty() && !id.is_empty()
                        }) =>
                    {
                        warnings.push(format!(
                            "`model` ({model:?}) must be `provider/model-id`: \
                         the child uses the default model."
                        ));
                        None
                    }
                    model => model,
                };
                // Explicit `model`, else the profile's model, else
                // `agent.subagent_model`; none left means the user's default.
                // External agents choose their own model.
                let model = if is_native {
                    resolve_child_model(
                        model.as_deref(),
                        profile_default_model.as_deref(),
                        subagent_model.as_deref(),
                    )
                    .map(|(model, _source)| model)
                } else {
                    None
                };
                // What the registry and the sidebar call the child's model.
                let registered_model = match (&external_agent_id, &external_model) {
                    (Some(agent_id), Some(requested)) => Some(format!(
                        "{agent_id}/{}",
                        early_model.as_deref().unwrap_or(requested)
                    )),
                    _ => model.clone(),
                };
                let prompt = format!(
                    "{}{}",
                    child_header(&orchestrator_title, input.subagent_quota),
                    input.prompt
                );

                let child = create_child_thread(
                    panel,
                    window,
                    ChildThreadRequest {
                        title: title.clone().into(),
                        prompt: prompt.clone(),
                        agent_id: input.agent,
                        model: model.clone(),
                        profile,
                        use_new_worktree: input.worktree == WorktreeMode::New,
                        base_ref: input.base_ref,
                        worktree_name: input.worktree_name,
                        parent_thread_id: Some(orchestrator),
                        subagent_quota: Some(ChildSubagentQuota {
                            granted: input.subagent_quota,
                            log_dir: cx.update(|cx| orchestration_log_dir(cx)),
                        }),
                        defer_prompt: external_model.is_some(),
                    },
                    cx,
                )
                .await?;
                warnings.extend(child.warning);
                if let (Some(agent_id), Some(model)) = (&external_agent_id, &early_model) {
                    cx.update(|cx| {
                        persist_external_run_config(child.thread_id, agent_id, model, cx)
                    });
                }

                let agent_id = child.agent_id.0.to_string();
                let registry = cx.update(OrchestrationRegistry::global_or_init);
                let log_dir = cx.update(|cx| orchestration_log_dir(cx));
                let log_path = registry.update(cx, |registry, cx| {
                    // The child now counts as running, so it no longer needs its
                    // reserved slot.
                    registry.release_child_slot();
                    registry.register_child(
                        orchestrator,
                        NewChild {
                            child_id: child.thread_id,
                            title: title.into(),
                            agent_id: agent_id.clone(),
                            model: registered_model,
                            log_dir,
                            prompt: input.prompt,
                            orchestrator_title,
                        },
                        cx,
                    )
                });
                cx.update(|cx| {
                    observe_child(
                        &children,
                        window,
                        orchestrator,
                        child.thread_id,
                        &child.conversation_view,
                        cx,
                    );
                    if let Some(requested) = external_model {
                        hold_prompt_for_model(
                            &children,
                            window,
                            orchestrator,
                            child.thread_id,
                            ModelGate {
                                agent_id: agent_id.clone(),
                                requested,
                                started: false,
                            },
                            prompt,
                            cx,
                        );
                    }
                    watch_orchestrator(&orchestrator_watches, orchestrator, &orchestrator_view, cx);
                });

                Ok(StartChildOutput {
                    child_id: child.thread_id.to_key_string(),
                    log_path: log_path.display().to_string(),
                    agent: agent_id,
                    warning: (!warnings.is_empty()).then(|| warnings.join(" ")),
                })
            }
            .await;
            if result.is_err() {
                slot_registry
                    .update(cx, |registry, _| registry.release_child_slot())
                    .ok();
            }
            result
        })
    }

    fn send_to_child(
        &self,
        orchestrator: ThreadId,
        input: SendToChildInput,
        cx: &mut App,
    ) -> Task<Result<SendToChildOutput>> {
        let window = self.window;
        let children = self.children.clone();
        let child = (|| {
            Self::ensure_enabled(cx)?;
            OrchestrationRegistry::global_or_init(cx)
                .read(cx)
                .resolve_child_id(orchestrator, &input.child_id)
        })();
        let child = match child {
            Ok(child) => child,
            Err(error) => return Task::ready(Err(error)),
        };
        let opening = self.open_child(orchestrator, child, cx);
        // Deferred, because sending needs the window and the caller may be
        // running inside an update of it.
        cx.spawn(async move |cx| {
            let conversation_view = opening.await?;
            let queued = send_or_queue(
                &children,
                window,
                child,
                &conversation_view,
                input.message.clone(),
                MessageSender {
                    role: SenderRole::Orchestrator,
                    thread: orchestrator,
                },
                cx,
            )?;
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
            // A child that orchestrates takes its running children with it,
            // even when its own turn already ended.
            OrchestrationRegistry::cancel_running_descendants(&registry, child, cx);
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

    fn cancel_thread_turn(&self, thread: ThreadId, cx: &mut App) -> Option<Option<String>> {
        let conversation_view = self.child_view(thread).ok()?;
        let acp_thread = conversation_view.read(cx).root_thread(cx)?;
        let final_message = acp_thread.read(cx).last_assistant_message(cx);
        conversation_view.update(cx, |view, cx| view.cancel_generation(cx));
        Some(final_message)
    }

    fn load_child(
        &self,
        orchestrator: ThreadId,
        child: ThreadId,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let opening = self.open_child(orchestrator, child, cx);
        cx.spawn(async move |_cx| {
            opening.await?;
            Ok(())
        })
    }

    fn list_available_agents(&self, cx: &mut App) -> Result<AvailableAgents> {
        let panel = self
            .panel
            .upgrade()
            .ok_or_else(|| anyhow!("Agent panel is no longer available"))?;
        let available = panel.read(cx).available_agents(cx);
        Ok(AvailableAgents {
            profiles: available.profiles,
            agents: available
                .agents
                .into_iter()
                .map(|agent| AvailableAgent {
                    id: agent.id,
                    name: agent.name,
                    is_native: agent.is_native,
                    models_note: agent.models_note,
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

    fn thread_title(&self, thread_id: ThreadId, cx: &App) -> Option<String> {
        self.owned_thread(thread_id, cx)?
            .read(cx)
            .title()
            .map(|title| title.to_string())
    }

    fn is_thread_idle(&self, thread_id: ThreadId, cx: &App) -> bool {
        self.owned_thread(thread_id, cx)
            .is_some_and(|thread| thread.read(cx).status() == ThreadStatus::Idle)
    }

    fn deliver_message(
        &self,
        thread_id: ThreadId,
        from: MessageSender,
        text: String,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let Some(conversation_view) = self.panel.upgrade().and_then(|panel| {
            panel
                .read(cx)
                .conversation_view_for_id(&thread_id, cx)
                .cloned()
        }) else {
            return Task::ready(Err(anyhow!("The thread is no longer open.")));
        };
        let window = self.window;
        // Deferred, because sending needs the window and the caller may be
        // running inside an update of it.
        cx.spawn(async move |cx| {
            send_text(window, &conversation_view, text, from, cx)?;
            Ok(())
        })
    }

    fn summarize_human_interventions(
        &self,
        request: HumanSummaryRequest,
        cx: &mut App,
    ) -> Task<Result<Option<String>>> {
        let Some(model) = LanguageModelRegistry::read_global(cx).thread_summary_model(cx) else {
            return Task::ready(Ok(None));
        };
        let Some(thread) = self.owned_thread(request.child, cx) else {
            return Task::ready(Err(anyhow!("The child thread is no longer open.")));
        };
        let transcript = human_turn_transcript(thread.read(cx), &request, cx);
        let language_request = LanguageModelRequest {
            intent: Some(CompletionIntent::ThreadSummarization),
            temperature: AgentSettings::temperature_for_model(&model, cx),
            messages: vec![LanguageModelRequestMessage {
                role: Role::User,
                content: vec![human_summary_prompt(&transcript, request.max_chars).into()],
                cache: false,
                reasoning_details: None,
            }],
            ..Default::default()
        };
        cx.spawn(async move |cx| {
            let provider =
                cx.update(|cx| LanguageModelRegistry::read_global(cx).provider_for_model(&model))?;
            let mut events = provider
                .stream_completion(&model, language_request, cx)
                .await?;
            let mut summary = String::new();
            while let Some(event) = events.next().await {
                if let LanguageModelCompletionEvent::Text(text) = event? {
                    summary.push_str(&text);
                }
            }
            Ok(Some(summary))
        })
    }
}

impl AgentPanelOrchestrationHost {
    /// The root ACP thread of a thread that this host's panel owns.
    fn owned_thread(&self, thread_id: ThreadId, cx: &App) -> Option<Entity<AcpThread>> {
        self.panel
            .upgrade()?
            .read(cx)
            .conversation_view_for_id(&thread_id, cx)?
            .read(cx)
            .root_thread(cx)
    }
}

/// Sends `text` to a child, or parks it in the host when the child's agent is
/// still connecting, and sends it as soon as the thread exists. Returns
/// whether the message was queued (parked, or held by a generating thread).
fn send_or_queue(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    window: gpui::AnyWindowHandle,
    child: ThreadId,
    conversation_view: &Entity<ConversationView>,
    text: String,
    from: MessageSender,
    cx: &mut gpui::AsyncApp,
) -> Result<bool> {
    let connecting = conversation_view.read_with(cx, |view, cx| view.root_thread(cx).is_none());
    let waiting_for_model = children
        .borrow()
        .get(&child)
        .is_some_and(|handle| handle.model_gate.is_some());
    if connecting || waiting_for_model {
        let mut children = children.borrow_mut();
        let handle = children
            .get_mut(&child)
            .context("The child thread is no longer tracked, so the message cannot be queued.")?;
        handle.pending.push(text);
        return Ok(true);
    }
    send_text(window, conversation_view, text, from, cx)
}

/// Sends the messages parked for a child whose thread now exists. A message
/// that cannot be sent yet stays parked for the next try.
fn flush_pending(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    window: gpui::AnyWindowHandle,
    child: ThreadId,
    cx: &mut App,
) {
    let (conversation_view, messages, orchestrator) = {
        let mut children = children.borrow_mut();
        let Some(handle) = children.get_mut(&child) else {
            return;
        };
        // The first prompt of a child with a model waits for the model.
        if handle.pending.is_empty() || handle.model_gate.is_some() {
            return;
        }
        let Some(conversation_view) = handle.conversation_view.upgrade() else {
            return;
        };
        if conversation_view.read(cx).root_thread(cx).is_none() {
            return;
        }
        (
            conversation_view,
            std::mem::take(&mut handle.pending),
            handle.orchestrator,
        )
    };
    let children = children.clone();
    cx.spawn(async move |cx| {
        let mut messages = messages.into_iter();
        while let Some(text) = messages.next() {
            if let Err(error) = send_text(
                window,
                &conversation_view,
                text.clone(),
                MessageSender {
                    role: SenderRole::Orchestrator,
                    thread: orchestrator,
                },
                cx,
            ) {
                log::error!("failed to send a queued message to a child: {error:#}");
                if let Some(handle) = children.borrow_mut().get_mut(&child) {
                    handle.pending.insert(0, text);
                    handle.pending.extend(messages);
                }
                return;
            }
        }
    })
    .detach();
}

/// Sends `text` to a thread as a user message that the orchestration tools
/// wrote. A thread that is generating queues it. Returns whether it was queued.
fn send_text(
    window: gpui::AnyWindowHandle,
    conversation_view: &Entity<ConversationView>,
    text: String,
    from: MessageSender,
    cx: &mut gpui::AsyncApp,
) -> Result<bool> {
    let content = vec![acp_v2::ContentBlock::Text(acp_v2::TextContent::new(text))];
    window.update(cx, |_root, window, cx| {
        conversation_view.update(cx, |view, cx| {
            view.send_follow_up(content, from, window, cx)
        })
    })?
}

/// The turns a summary covers, as text for the summary model. Messages are cut
/// and the whole is capped, because a human may steer a child through hundreds
/// of turns. Tool calls and thoughts are left out.
fn human_turn_transcript(thread: &AcpThread, request: &HumanSummaryRequest, cx: &App) -> String {
    const MAX_MESSAGE_CHARS: usize = 1_500;
    const MAX_TRANSCRIPT_CHARS: usize = 12_000;

    let mut transcript = String::new();
    for (index, entry) in thread.entries().iter().enumerate().skip(request.from_entry) {
        let (speaker, text) = match entry {
            AgentThreadEntry::UserMessage(message) => {
                let speaker = if request.orchestrator_entries.contains(&index) {
                    "Orchestrator"
                } else {
                    "Human"
                };
                (speaker, message.content.to_markdown(cx).to_string())
            }
            AgentThreadEntry::AssistantMessage(message) if !message.is_subagent_output => {
                let text = message
                    .chunks
                    .iter()
                    .filter_map(|chunk| match chunk {
                        AssistantMessageChunk::Message { block, .. } => {
                            Some(block.to_markdown(cx).to_string())
                        }
                        AssistantMessageChunk::Thought { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                ("Agent", text)
            }
            _ => continue,
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let text = util::truncate_and_trailoff(text, MAX_MESSAGE_CHARS);
        transcript.push_str(&format!("{speaker}: {text}\n\n"));
        if transcript.chars().count() >= MAX_TRANSCRIPT_CHARS {
            transcript.push_str("[the rest is left out]\n");
            break;
        }
    }
    transcript
}

/// Delivers urgent child messages when the orchestrator thread stops. The
/// thread is watched once, however many children it starts.
fn watch_orchestrator(
    watches: &Rc<RefCell<HashMap<ThreadId, Subscription>>>,
    orchestrator: ThreadId,
    conversation_view: &Entity<ConversationView>,
    cx: &mut App,
) {
    if watches.borrow().contains_key(&orchestrator) {
        return;
    }
    let Some(thread) = conversation_view.read(cx).root_thread(cx) else {
        return;
    };
    let subscription = cx.subscribe(&thread, move |_thread, event, cx| {
        if matches!(
            event,
            AcpThreadEvent::Stopped { .. }
                | AcpThreadEvent::Error
                | AcpThreadEvent::ConnectionLost
                | AcpThreadEvent::StatusChanged
                | AcpThreadEvent::ActivityConfirmedIdle
        ) {
            let registry = OrchestrationRegistry::global_or_init(cx);
            OrchestrationRegistry::flush_urgent(&registry, orchestrator, cx);
        }
    });
    watches.borrow_mut().insert(orchestrator, subscription);
}

/// Starts watching a child: its `ConversationView` for the moment its root
/// thread exists (or the agent fails to load), and then the `AcpThread`.
fn observe_child(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    window: gpui::AnyWindowHandle,
    orchestrator: ThreadId,
    child: ThreadId,
    conversation_view: &Entity<ConversationView>,
    cx: &mut App,
) {
    let root_subscription = cx.subscribe(conversation_view, {
        let children = children.clone();
        move |conversation_view, _: &RootThreadUpdated, cx| {
            attach_thread_observer(
                &children,
                window,
                orchestrator,
                child,
                &conversation_view,
                cx,
            );
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
    cx.default_global::<ChildConversations>()
        .0
        .insert(child, conversation_view.downgrade());
    // Messages parked while the thread was not connected survive a re-observe.
    let (pending, model_gate) = children
        .borrow_mut()
        .remove(&child)
        .map(|handle| (handle.pending, handle.model_gate))
        .unwrap_or_default();
    children.borrow_mut().insert(
        child,
        ChildHandle {
            conversation_view: conversation_view.downgrade(),
            thread_subscription: None,
            _subscriptions: vec![root_subscription, load_subscription],
            pending,
            orchestrator,
            model_gate,
        },
    );
    attach_thread_observer(children, window, orchestrator, child, conversation_view, cx);
}

/// Parks the first prompt of an external child until its model is set (R2).
/// The model is applied when the child's session exists, and the prompt is
/// sent after it, or anyway after [`orchestration_models::MODEL_APPLY_TIMEOUT`].
fn hold_prompt_for_model(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    window: gpui::AnyWindowHandle,
    orchestrator: ThreadId,
    child: ThreadId,
    gate: ModelGate,
    prompt: String,
    cx: &mut App,
) {
    {
        let mut children = children.borrow_mut();
        let Some(handle) = children.get_mut(&child) else {
            return;
        };
        handle.pending.insert(0, prompt);
        handle.model_gate = Some(gate);
    }
    start_model_gate(children, window, orchestrator, child, cx);
    flush_pending(children, window, child, cx);
}

/// Starts setting the model of a child whose session exists. Runs once.
fn start_model_gate(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    window: gpui::AnyWindowHandle,
    orchestrator: ThreadId,
    child: ThreadId,
    cx: &mut App,
) {
    let (thread, agent_id, requested) = {
        let mut children = children.borrow_mut();
        let Some(handle) = children.get_mut(&child) else {
            return;
        };
        let Some(gate) = handle.model_gate.as_mut().filter(|gate| !gate.started) else {
            return;
        };
        let Some(thread) = handle
            .conversation_view
            .upgrade()
            .and_then(|view| view.read(cx).root_thread(cx))
        else {
            return;
        };
        gate.started = true;
        (thread, gate.agent_id.clone(), gate.requested.clone())
    };
    let options = {
        let thread = thread.read(cx);
        thread
            .connection()
            .session_config_options(thread.session_id(), cx)
    };
    let timeout = cx
        .background_executor()
        .timer(orchestration_models::MODEL_APPLY_TIMEOUT);
    let children = children.clone();
    cx.spawn(async move |cx| {
        let outcome = orchestration_models::apply_model_before_prompt(
            options, &agent_id, &requested, timeout, cx,
        )
        .await;
        cx.update(|cx| {
            finish_model_gate(
                &children,
                window,
                orchestrator,
                child,
                &agent_id,
                outcome,
                cx,
            )
        });
    })
    .detach();
}

/// Ends the wait for the model: sends the first prompt, or fails the child.
fn finish_model_gate(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    window: gpui::AnyWindowHandle,
    orchestrator: ThreadId,
    child: ThreadId,
    agent_id: &str,
    outcome: ModelGateOutcome,
    cx: &mut App,
) {
    let waiting = children
        .borrow_mut()
        .get_mut(&child)
        .and_then(|handle| handle.model_gate.take())
        .is_some();
    if !waiting {
        return;
    }
    match outcome {
        ModelGateOutcome::Applied(applied) => {
            persist_external_run_config(child, agent_id, &applied.value, cx);
            flush_pending(children, window, child, cx);
        }
        ModelGateOutcome::TimedOut => {
            log::warn!(
                "start_child: agent {agent_id:?} did not confirm the model in time, so the \
                 first prompt of the child was sent without it"
            );
            flush_pending(children, window, child, cx);
        }
        ModelGateOutcome::Rejected(message) => {
            if let Some(handle) = children.borrow_mut().get_mut(&child) {
                handle.pending.clear();
            }
            finish_turn(orchestrator, child, ChildStatus::Failed, Some(message), cx);
        }
    }
}

/// Stores the model an external child runs on in its thread metadata, with
/// the agent as the provider, so the sidebar can show it while the child is
/// not loaded.
fn persist_external_run_config(thread_id: ThreadId, agent_id: &str, model_id: &str, cx: &mut App) {
    let Some(store) = ThreadMetadataStore::try_global(cx) else {
        return;
    };
    let agent_id: gpui::SharedString = agent_id.to_string().into();
    let model_id: gpui::SharedString = model_id.to_string().into();
    store.update(cx, |store, cx| {
        store.update_run_config(
            thread_id,
            |config| {
                config.model_provider_id = Some(agent_id);
                config.model_id = Some(model_id);
            },
            cx,
        );
    });
}

fn attach_thread_observer(
    children: &Rc<RefCell<HashMap<ThreadId, ChildHandle>>>,
    window: gpui::AnyWindowHandle,
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
    let subscription = cx.subscribe(&thread, {
        let children = children.clone();
        move |thread, event, cx| {
            on_thread_event(orchestrator, child, &thread, event, cx);
            // Parked and sibling messages wait for the child to be between
            // turns, so they go after the turn's end is recorded.
            if matches!(
                event,
                AcpThreadEvent::Stopped { .. }
                    | AcpThreadEvent::StatusChanged
                    | AcpThreadEvent::ActivityConfirmedIdle
            ) {
                flush_pending(&children, window, child, cx);
                OrchestrationRegistry::flush_sibling_inbox(
                    &OrchestrationRegistry::global_or_init(cx),
                    child,
                    cx,
                );
            }
        }
    });
    if let Some(handle) = children.borrow_mut().get_mut(&child) {
        handle.thread_subscription = Some(subscription);
    }
    sync_live_status(orchestrator, child, &thread, cx);
    start_model_gate(children, window, orchestrator, child, cx);
    flush_pending(children, window, child, cx);
    OrchestrationRegistry::flush_sibling_inbox(
        &OrchestrationRegistry::global_or_init(cx),
        child,
        cx,
    );
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
