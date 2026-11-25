use std::{path::PathBuf, rc::Rc, time::Duration};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use collections::HashMap;
use futures::{FutureExt as _, future};
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, Global, ReadGlobal as _, SharedString,
    Task,
};
use settings::SettingsStore;

use crate::{
    AwaitChildrenInput, AwaitChildrenOutput, ChildLog, ChildReference, ChildResult, ChildStatus,
    ChildStatusInfo, ChildStatusInput, ChildStatusOutput, DEFAULT_AWAIT_TIMEOUT_SECS,
    HumanIntervention, HumanSummaryRequest, LogDirection, MAX_AWAIT_TIMEOUT_SECS,
    OrchestrationHost, PendingMessage, SendToParentInput, SendToParentOutput, ThreadId,
    ThreadReference, ThreadRole, WhoAmIOutput, human_summary_fallback, log_dir_from_setting,
    truncate_summary, urgent_message_prefix,
};

/// Default of `agent.orchestration.human_summary_max_chars`.
pub const DEFAULT_HUMAN_SUMMARY_MAX_CHARS: usize = 600;

/// What the registry needs to start tracking a child.
pub struct NewChild {
    pub child_id: ThreadId,
    pub title: SharedString,
    /// The agent that runs the child.
    pub agent_id: String,
    pub model: Option<String>,
    /// Root directory of the logs, see [`orchestration_log_dir`].
    pub log_dir: PathBuf,
    /// The child's first message, written to its log as the first `to_child`
    /// line.
    pub prompt: String,
    /// Title of the orchestrator thread when the child started, for the child's
    /// `whoami` when the live title is not available.
    pub orchestrator_title: String,
}

/// The orchestrator's view of one child. It never holds the child's
/// transcript, only the last message of its latest turn.
pub struct ChildRecord {
    pub child_id: ThreadId,
    pub title: SharedString,
    pub agent_id: String,
    pub model: Option<String>,
    pub status: ChildStatus,
    /// The last assistant message of the latest finished turn.
    pub final_message: Option<String>,
    /// Number of turns the child has finished.
    pub turns: u32,
    pub last_activity: DateTime<Utc>,
    pub log_path: PathBuf,
    pub orchestrator_title: String,
    log: ChildLog,
    status_sender: watch::Sender<ChildStatus>,
    /// What the child sent with `send_to_parent`.
    inbox: Vec<InboxMessage>,
    /// Counts the urgent messages, so `await_children` can wake on a new one.
    urgent_count: u32,
    urgent_sender: watch::Sender<u32>,
    /// Human-driven turns since the last summary. The human's messages
    /// themselves are never stored.
    human_turns: u32,
    /// Index of the first thread entry the next summary covers.
    human_cursor: Option<usize>,
    /// User messages the orchestrator sent after `human_cursor`, so the
    /// summary does not credit them to the human.
    orchestrator_entries: Vec<usize>,
    /// Summaries the orchestrator has not read yet.
    interventions: Vec<HumanIntervention>,
}

struct InboxMessage {
    id: u64,
    sent_at: DateTime<Utc>,
    text: String,
    urgent: bool,
    /// Seen by the orchestrator, through `pending_messages` or as a message
    /// in its thread.
    delivered: bool,
}

impl ChildRecord {
    fn has_undelivered_urgent(&self) -> bool {
        self.inbox
            .iter()
            .any(|message| message.urgent && !message.delivered)
    }

    /// The messages the orchestrator has not seen. Each is returned once.
    fn take_pending_messages(&mut self) -> Vec<PendingMessage> {
        self.inbox
            .iter_mut()
            .filter(|message| !message.delivered)
            .map(|message| {
                message.delivered = true;
                PendingMessage {
                    message: message.text.clone(),
                    sent_at: message.sent_at.to_rfc3339(),
                    urgent: message.urgent,
                }
            })
            .collect()
    }

    /// A receiver that observes every status change of this child.
    pub fn subscribe_status(&self) -> watch::Receiver<ChildStatus> {
        self.status_sender.receiver()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrchestrationRegistryEvent {
    /// A child was added, changed status, or exchanged a message.
    ChildChanged {
        orchestrator: ThreadId,
        child: ThreadId,
    },
}

struct GlobalOrchestrationRegistry(Entity<OrchestrationRegistry>);

impl Global for GlobalOrchestrationRegistry {}

/// The directory for child logs, from `agent.orchestration.log_dir`.
pub fn orchestration_log_dir(cx: &App) -> PathBuf {
    // `log_dir` is not part of `AgentSettings`: a path is not `Copy`, and the
    // sidebar copies `AgentSettings::orchestration` by value.
    let log_dir = SettingsStore::global(cx)
        .merged_settings()
        .agent
        .as_ref()
        .and_then(|agent| agent.orchestration.as_ref())
        .and_then(|orchestration| orchestration.log_dir.as_deref());
    log_dir_from_setting(log_dir)
}

/// The cap on the length of human-intervention summaries, from
/// `agent.orchestration.human_summary_max_chars`.
pub fn human_summary_max_chars(cx: &App) -> usize {
    cx.try_global::<SettingsStore>()
        .and_then(|store| {
            store
                .merged_settings()
                .agent
                .as_ref()
                .and_then(|agent| agent.orchestration.as_ref())
                .and_then(|orchestration| orchestration.human_summary_max_chars)
        })
        .unwrap_or(DEFAULT_HUMAN_SUMMARY_MAX_CHARS)
}

/// Tracks every child of every orchestrator in the app.
///
/// It is also where callers learn who they are acting for: a token minted
/// here identifies an orchestrator thread, and registered hosts resolve ACP
/// sessions to threads.
#[derive(Default)]
pub struct OrchestrationRegistry {
    children: HashMap<ThreadId, Vec<ChildRecord>>,
    tokens: HashMap<String, ThreadId>,
    /// Tokens handed to agent sessions, mapped to the session they belong to
    /// (`None` until the session exists). Unlike `tokens`, they are minted
    /// before the Zed thread is known: the code that creates ACP sessions
    /// knows only the session id. See [`Self::resolve_caller`].
    session_tokens: HashMap<String, Option<String>>,
    hosts: Vec<Rc<dyn OrchestrationHost>>,
    next_message_id: u64,
}

impl EventEmitter<OrchestrationRegistryEvent> for OrchestrationRegistry {}

impl OrchestrationRegistry {
    /// Creates the global registry if it does not exist yet, and returns it.
    pub fn global_or_init(cx: &mut App) -> Entity<Self> {
        if let Some(global) = cx.try_global::<GlobalOrchestrationRegistry>() {
            return global.0.clone();
        }
        let registry = cx.new(|_| Self::default());
        cx.set_global(GlobalOrchestrationRegistry(registry.clone()));
        registry
    }

    /// The global registry. Panics if [`Self::global_or_init`] never ran.
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalOrchestrationRegistry>().0.clone()
    }

    pub fn try_global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<GlobalOrchestrationRegistry>()
            .map(|global| global.0.clone())
    }

    /// Mints the token that identifies `orchestrator` to callers outside the
    /// app (the MCP server). Calling it again for the same thread returns the
    /// same token.
    pub fn mint_token(&mut self, orchestrator: ThreadId) -> String {
        if let Some((token, _)) = self
            .tokens
            .iter()
            .find(|(_, thread)| **thread == orchestrator)
        {
            return token.clone();
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        self.tokens.insert(token.clone(), orchestrator);
        token
    }

    /// The orchestrator a token was minted for.
    pub fn resolve_token(&self, token: &str) -> Option<ThreadId> {
        self.tokens.get(token).copied()
    }

    /// Mints a token for a session that is about to be created, before its
    /// session id exists. Call [`Self::bind_session_token`] once it does.
    pub fn mint_pending_session_token(&mut self) -> String {
        let token = uuid::Uuid::new_v4().simple().to_string();
        self.session_tokens.insert(token.clone(), None);
        token
    }

    /// Ties a token to the ACP session it was given to.
    pub fn bind_session_token(&mut self, token: &str, session_id: &str) {
        if let Some(session) = self.session_tokens.get_mut(token) {
            *session = Some(session_id.to_string());
        }
    }

    /// The token of an existing session, minted on first use. Loading or
    /// resuming the same session again returns the same token, so the MCP
    /// server entry stays identical.
    pub fn token_for_session(&mut self, session_id: &str) -> String {
        if let Some((token, _)) = self
            .session_tokens
            .iter()
            .find(|(_, session)| session.as_deref() == Some(session_id))
        {
            return token.clone();
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        self.session_tokens
            .insert(token.clone(), Some(session_id.to_string()));
        token
    }

    /// Resolves the token an MCP connection presented to the orchestrator
    /// thread it speaks for, and the host that owns that thread. A token that
    /// was never minted, or whose session is not an orchestrator's root
    /// session, resolves to `None`.
    pub fn resolve_caller(
        &self,
        token: &str,
        cx: &App,
    ) -> Option<(ThreadId, Rc<dyn OrchestrationHost>)> {
        if let Some(thread) = self.tokens.get(token) {
            return self
                .host_for_thread(*thread, cx)
                .map(|host| (*thread, host));
        }
        let session = self.session_tokens.get(token)?.as_deref()?;
        self.host_for_session(session, cx)
    }

    pub fn revoke_tokens(&mut self, orchestrator: ThreadId) {
        self.tokens.retain(|_, thread| *thread != orchestrator);
    }

    /// Registers a host (one per agent panel). Registering the same host
    /// twice is a no-op.
    pub fn register_host(&mut self, host: Rc<dyn OrchestrationHost>) {
        if !self.hosts.iter().any(|existing| same_host(existing, &host)) {
            self.hosts.push(host);
        }
    }

    pub fn unregister_host(&mut self, host: &Rc<dyn OrchestrationHost>) {
        self.hosts.retain(|existing| !same_host(existing, host));
    }

    /// Finds the orchestrator whose root session is `session_id`, with the
    /// host that owns it.
    pub fn host_for_session(
        &self,
        session_id: &str,
        cx: &App,
    ) -> Option<(ThreadId, Rc<dyn OrchestrationHost>)> {
        self.hosts.iter().find_map(|host| {
            host.resolve_orchestrator(session_id, cx)
                .map(|thread| (thread, host.clone()))
        })
    }

    /// The host that owns the thread `orchestrator`, for callers that
    /// identified it with a token.
    pub fn host_for_thread(
        &self,
        orchestrator: ThreadId,
        cx: &App,
    ) -> Option<Rc<dyn OrchestrationHost>> {
        self.hosts
            .iter()
            .find(|host| host.owns_thread(orchestrator, cx))
            .cloned()
    }

    /// Starts tracking a child and writes its first prompt to its log.
    /// Returns the path of the log.
    pub fn register_child(
        &mut self,
        orchestrator: ThreadId,
        child: NewChild,
        cx: &mut Context<Self>,
    ) -> PathBuf {
        let log = ChildLog::new(&child.log_dir, orchestrator, child.child_id);
        let log_path = log.path().to_path_buf();
        log_exchange(
            &log,
            LogDirection::ToChild,
            &child.prompt,
            ChildStatus::Starting,
        );
        let (status_sender, _) = watch::channel(ChildStatus::Starting);
        let (urgent_sender, _) = watch::channel(0);
        let child_id = child.child_id;
        self.children
            .entry(orchestrator)
            .or_default()
            .push(ChildRecord {
                child_id,
                title: child.title,
                agent_id: child.agent_id,
                model: child.model,
                status: ChildStatus::Starting,
                final_message: None,
                turns: 0,
                last_activity: Utc::now(),
                log_path: log_path.clone(),
                orchestrator_title: child.orchestrator_title,
                log,
                status_sender,
                inbox: Vec::new(),
                urgent_count: 0,
                urgent_sender,
                human_turns: 0,
                human_cursor: None,
                orchestrator_entries: Vec::new(),
                interventions: Vec::new(),
            });
        cx.emit(OrchestrationRegistryEvent::ChildChanged {
            orchestrator,
            child: child_id,
        });
        cx.notify();
        log_path
    }

    /// Forgets an orchestrator, its children and its tokens, when its thread
    /// is gone. The logs stay on disk.
    pub fn remove_orchestrator(&mut self, orchestrator: ThreadId, cx: &mut Context<Self>) {
        self.revoke_tokens(orchestrator);
        if self.children.remove(&orchestrator).is_some() {
            cx.notify();
        }
    }

    /// The children of `orchestrator`, in the order they were started.
    pub fn children(&self, orchestrator: ThreadId) -> &[ChildRecord] {
        self.children
            .get(&orchestrator)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn child(&self, orchestrator: ThreadId, child: ThreadId) -> Option<&ChildRecord> {
        self.children(orchestrator)
            .iter()
            .find(|record| record.child_id == child)
    }

    /// The orchestrator that started `child`.
    pub fn orchestrator_of(&self, child: ThreadId) -> Option<ThreadId> {
        self.children.iter().find_map(|(orchestrator, records)| {
            records
                .iter()
                .any(|record| record.child_id == child)
                .then_some(*orchestrator)
        })
    }

    /// Parses `child_ids` and checks that each belongs to `orchestrator`. An
    /// empty list means every child of the orchestrator.
    pub fn resolve_child_ids(
        &self,
        orchestrator: ThreadId,
        child_ids: &[String],
    ) -> Result<Vec<ThreadId>> {
        if child_ids.is_empty() {
            return Ok(self
                .children(orchestrator)
                .iter()
                .map(|record| record.child_id)
                .collect());
        }
        child_ids
            .iter()
            .map(|raw| self.resolve_child_id(orchestrator, raw))
            .collect()
    }

    pub fn resolve_child_id(&self, orchestrator: ThreadId, raw: &str) -> Result<ThreadId> {
        ThreadId::from_key_string(raw)
            .filter(|child| self.child(orchestrator, *child).is_some())
            .ok_or_else(|| anyhow!("Unknown child_id {raw:?}: it was not started by this thread."))
    }

    /// Updates the status of a child and wakes whoever waits on it.
    pub fn set_status(
        &mut self,
        orchestrator: ThreadId,
        child: ThreadId,
        status: ChildStatus,
        cx: &mut Context<Self>,
    ) {
        let Some(record) = self.child_mut(orchestrator, child) else {
            return;
        };
        if record.status == status {
            return;
        }
        record.apply_status(status);
        cx.emit(OrchestrationRegistryEvent::ChildChanged {
            orchestrator,
            child,
        });
        cx.notify();
    }

    /// Records a message from the orchestrator to the child (a follow-up).
    /// A child that had finished starts a new turn, so it goes back to
    /// `Running`.
    pub fn record_message_to_child(
        &mut self,
        orchestrator: ThreadId,
        child: ThreadId,
        text: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(record) = self.child_mut(orchestrator, child) else {
            return;
        };
        if record.status.is_terminal() {
            record.apply_status(ChildStatus::Running);
        }
        log_exchange(&record.log, LogDirection::ToChild, text, record.status);
        cx.emit(OrchestrationRegistryEvent::ChildChanged {
            orchestrator,
            child,
        });
        cx.notify();
    }

    /// Records the end of a child's turn: counts it, keeps the final message,
    /// writes it to the log and moves the child to `status`, which should be
    /// terminal.
    pub fn record_turn_finished(
        &mut self,
        orchestrator: ThreadId,
        child: ThreadId,
        status: ChildStatus,
        final_message: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(record) = self.child_mut(orchestrator, child) else {
            return;
        };
        record.turns += 1;
        log_exchange(
            &record.log,
            LogDirection::FromChild,
            final_message.as_deref().unwrap_or_default(),
            status,
        );
        record.final_message = final_message;
        record.apply_status(status);
        cx.emit(OrchestrationRegistryEvent::ChildChanged {
            orchestrator,
            child,
        });
        cx.notify();
    }

    fn child_mut(&mut self, orchestrator: ThreadId, child: ThreadId) -> Option<&mut ChildRecord> {
        self.children
            .get_mut(&orchestrator)?
            .iter_mut()
            .find(|record| record.child_id == child)
    }

    /// Records a message from `child` to its orchestrator: it goes to the
    /// child's log and the orchestrator's inbox. Returns the orchestrator.
    pub fn record_message_from_child(
        &mut self,
        child: ThreadId,
        text: &str,
        urgent: bool,
        cx: &mut Context<Self>,
    ) -> Result<ThreadId> {
        let orchestrator = self.orchestrator_of(child).ok_or_else(|| {
            anyhow!(
                "This thread has no orchestrator, so there is nobody to send a message to. \
                 Reply in your own chat instead."
            )
        })?;
        let id = self.next_message_id;
        self.next_message_id += 1;
        let record = self
            .child_mut(orchestrator, child)
            .ok_or_else(|| anyhow!("This thread is no longer tracked by its orchestrator."))?;
        log_exchange(&record.log, LogDirection::ToParent, text, record.status);
        record.inbox.push(InboxMessage {
            id,
            sent_at: Utc::now(),
            text: text.to_string(),
            urgent,
            delivered: false,
        });
        if urgent {
            record.urgent_count += 1;
            let count = record.urgent_count;
            record.urgent_sender.send(count).ok();
        }
        record.last_activity = Utc::now();
        cx.emit(OrchestrationRegistryEvent::ChildChanged {
            orchestrator,
            child,
        });
        cx.notify();
        Ok(orchestrator)
    }

    /// Notes that the human wrote in `child` directly. Only a counter and the
    /// position of the first affected thread entry are kept: the human's
    /// message is neither logged nor forwarded. A thread that is not a child
    /// is ignored.
    pub fn note_human_turn(&mut self, child: ThreadId, first_entry: usize) {
        let Some(orchestrator) = self.orchestrator_of(child) else {
            return;
        };
        if let Some(record) = self.child_mut(orchestrator, child) {
            record.human_turns += 1;
            record.human_cursor.get_or_insert(first_entry);
        }
    }

    /// Notes that the orchestrator sent `child` a message while a human
    /// intervention is waiting for its summary.
    pub fn note_orchestrator_turn(&mut self, child: ThreadId, entry: usize) {
        let Some(orchestrator) = self.orchestrator_of(child) else {
            return;
        };
        if let Some(record) = self.child_mut(orchestrator, child)
            && record.human_cursor.is_some()
        {
            record.orchestrator_entries.push(entry);
        }
    }

    /// Human-driven turns of `child` that no summary covers yet.
    pub fn pending_human_turns(&self, child: ThreadId) -> u32 {
        self.orchestrator_of(child)
            .and_then(|orchestrator| self.child(orchestrator, child))
            .map_or(0, |record| record.human_turns)
    }

    fn record_human_intervention(
        &mut self,
        orchestrator: ThreadId,
        child: ThreadId,
        turns: u32,
        summary: String,
        cx: &mut Context<Self>,
    ) {
        let Some(record) = self.child_mut(orchestrator, child) else {
            return;
        };
        log_exchange(
            &record.log,
            LogDirection::HumanIntervention,
            &summary,
            record.status,
        );
        record.interventions.push(HumanIntervention {
            summary,
            turns,
            summarized_at: Utc::now().to_rfc3339(),
        });
        cx.emit(OrchestrationRegistryEvent::ChildChanged {
            orchestrator,
            child,
        });
        cx.notify();
    }

    /// The role of `thread`, its orchestrator and its children.
    pub fn whoami(&self, thread: ThreadId, cx: &App) -> WhoAmIOutput {
        let children = self
            .children(thread)
            .iter()
            .map(|record| ChildReference {
                id: record.child_id.to_key_string(),
                title: record.title.to_string(),
                status: record.status,
            })
            .collect::<Vec<_>>();
        let parent = self.orchestrator_of(thread).map(|parent| {
            let live_title = self
                .host_for_thread(parent, cx)
                .and_then(|host| host.thread_title(parent, cx));
            let title = live_title
                .or_else(|| {
                    self.child(parent, thread)
                        .map(|record| record.orchestrator_title.clone())
                })
                .unwrap_or_default();
            ThreadReference {
                id: parent.to_key_string(),
                title,
            }
        });
        let role = match (parent.is_some(), !children.is_empty()) {
            (true, true) => ThreadRole::Both,
            (true, false) => ThreadRole::Child,
            (false, true) => ThreadRole::Orchestrator,
            (false, false) => ThreadRole::None,
        };
        WhoAmIOutput {
            role,
            parent,
            children,
        }
    }

    /// `send_to_parent`: records the message and, when it is urgent, tries to
    /// put it in the orchestrator's thread right away.
    pub fn send_to_parent(
        registry: &Entity<Self>,
        caller: ThreadId,
        input: SendToParentInput,
        cx: &mut App,
    ) -> Result<SendToParentOutput> {
        let message = input.message.trim();
        if message.is_empty() {
            return Err(anyhow!("`message` is empty."));
        }
        let orchestrator = registry.update(cx, |registry, cx| {
            registry.record_message_from_child(caller, message, input.urgent, cx)
        })?;
        if input.urgent {
            Self::flush_urgent(registry, orchestrator, cx);
        }
        let parent_title = registry
            .read(cx)
            .whoami(caller, cx)
            .parent
            .map(|parent| parent.title)
            .unwrap_or_default();
        Ok(SendToParentOutput {
            parent_id: orchestrator.to_key_string(),
            parent_title,
            urgent: input.urgent,
        })
    }

    /// Delivers the urgent messages that wait for `orchestrator` as one new
    /// message in its thread, but only when the thread is idle. A running turn
    /// is never interrupted: callers retry when the orchestrator stops.
    pub fn flush_urgent(registry: &Entity<Self>, orchestrator: ThreadId, cx: &mut App) {
        let Some(host) = registry.read(cx).host_for_thread(orchestrator, cx) else {
            return;
        };
        if !host.is_thread_idle(orchestrator, cx) {
            return;
        }
        let (text, delivered_ids) = registry.update(cx, |registry, _| {
            let mut parts = Vec::new();
            let mut ids = Vec::new();
            for record in registry
                .children
                .get_mut(&orchestrator)
                .into_iter()
                .flatten()
            {
                let prefix = urgent_message_prefix(&record.title);
                for message in record
                    .inbox
                    .iter_mut()
                    .filter(|message| message.urgent && !message.delivered)
                {
                    message.delivered = true;
                    ids.push(message.id);
                    parts.push(format!("{prefix} {}", message.text));
                }
            }
            (parts.join("\n\n"), ids)
        });
        if delivered_ids.is_empty() {
            return;
        }
        let delivery = host.deliver_message(orchestrator, text, cx);
        let registry = registry.downgrade();
        cx.spawn(async move |cx| {
            if let Err(error) = delivery.await {
                log::error!("failed to deliver an urgent child message: {error:#}");
                // Back to undelivered, so `pending_messages` still returns it.
                registry
                    .update(cx, |registry, _| {
                        for record in registry
                            .children
                            .get_mut(&orchestrator)
                            .into_iter()
                            .flatten()
                        {
                            for message in &mut record.inbox {
                                if delivered_ids.contains(&message.id) {
                                    message.delivered = false;
                                }
                            }
                        }
                    })
                    .ok();
            }
        })
        .detach();
    }

    /// Writes one summary for each requested child that has human-driven turns
    /// no summary covers yet. This is the only place a summary is generated,
    /// so a model is never called when the orchestrator does not ask.
    fn collect_human_interventions(
        registry: &Entity<Self>,
        orchestrator: ThreadId,
        children: &[ThreadId],
        cx: &mut App,
    ) -> Task<()> {
        let pending = registry.update(cx, |registry, _| {
            children
                .iter()
                .filter_map(|child| {
                    let record = registry.child_mut(orchestrator, *child)?;
                    if record.human_turns == 0 {
                        return None;
                    }
                    let request = (
                        *child,
                        record.human_turns,
                        record.human_cursor.unwrap_or_default(),
                        std::mem::take(&mut record.orchestrator_entries),
                    );
                    // Turns that happen while the summary is written count
                    // toward the next one.
                    record.human_turns = 0;
                    record.human_cursor = None;
                    Some(request)
                })
                .collect::<Vec<_>>()
        });
        if pending.is_empty() {
            return Task::ready(());
        }
        let max_chars = human_summary_max_chars(cx);
        let jobs = pending
            .into_iter()
            .map(|(child, turns, from_entry, orchestrator_entries)| {
                let host = registry.read(cx).host_for_thread(child, cx);
                let summary = host.map(|host| {
                    host.summarize_human_interventions(
                        HumanSummaryRequest {
                            child,
                            turns,
                            from_entry,
                            orchestrator_entries,
                            max_chars,
                        },
                        cx,
                    )
                });
                (child, turns, summary)
            })
            .collect::<Vec<_>>();
        let registry = registry.downgrade();
        cx.spawn(async move |cx| {
            let summaries =
                future::join_all(jobs.into_iter().map(|(child, turns, summary)| async move {
                    let text = match summary {
                        None => format!(
                            "A human intervened in {turns} turn(s); the child thread is closed, \
                             so no summary could be written."
                        ),
                        Some(task) => match task.await {
                            Ok(Some(text)) if !text.trim().is_empty() => {
                                truncate_summary(&text, max_chars)
                            }
                            Ok(_) => human_summary_fallback(turns),
                            Err(error) => {
                                log::error!("failed to summarize a human intervention: {error:#}");
                                format!(
                                    "A human intervened in {turns} turn(s); the summary could \
                                     not be written."
                                )
                            }
                        },
                    };
                    (child, turns, text)
                }))
                .await;
            registry
                .update(cx, |registry, cx| {
                    for (child, turns, text) in summaries {
                        registry.record_human_intervention(orchestrator, child, turns, text, cx);
                    }
                })
                .ok();
        })
    }

    /// Status, counters and times, never the child's content. It also hands
    /// over the messages and human-intervention summaries the orchestrator has
    /// not seen yet.
    fn take_child_status(
        &mut self,
        orchestrator: ThreadId,
        children: &[ThreadId],
    ) -> Vec<ChildStatusInfo> {
        let mut infos = Vec::new();
        for child in children {
            let Some(record) = self.child_mut(orchestrator, *child) else {
                continue;
            };
            infos.push(ChildStatusInfo {
                child_id: record.child_id.to_key_string(),
                title: record.title.to_string(),
                agent: record.agent_id.clone(),
                model: record.model.clone(),
                status: record.status,
                turns: record.turns,
                last_activity: record.last_activity.to_rfc3339(),
                pending_messages: record.take_pending_messages(),
                human_interventions: std::mem::take(&mut record.interventions),
            });
        }
        infos
    }

    pub fn child_status_with_summaries(
        registry: &Entity<Self>,
        orchestrator: ThreadId,
        input: ChildStatusInput,
        cx: &mut App,
    ) -> Task<Result<ChildStatusOutput>> {
        let child_ids = match registry
            .read(cx)
            .resolve_child_ids(orchestrator, &input.child_ids)
        {
            Ok(child_ids) => child_ids,
            Err(error) => return Task::ready(Err(error)),
        };
        let summaries = Self::collect_human_interventions(registry, orchestrator, &child_ids, cx);
        let registry = registry.downgrade();
        cx.spawn(async move |cx| {
            summaries.await;
            let children = registry.update(cx, |registry, _| {
                registry.take_child_status(orchestrator, &child_ids)
            })?;
            Ok(ChildStatusOutput { children })
        })
    }

    /// Waits until each requested child is in a terminal state, or until the
    /// timeout, and returns each child's status and final message only.
    ///
    /// Children that already finished return at once. Children that are
    /// still running when the timeout expires come back with their current
    /// status and no message, and `timed_out` set. A child that sends an urgent
    /// message wakes the wait early: the orchestrator is blocked in this call,
    /// so it could not see the message any other way.
    pub fn await_children(
        registry: &Entity<Self>,
        orchestrator: ThreadId,
        input: AwaitChildrenInput,
        cx: &mut App,
    ) -> Task<Result<AwaitChildrenOutput>> {
        let (child_ids, receivers, urgent_receivers, urgent_waiting) = {
            let this = registry.read(cx);
            let child_ids = match this.resolve_child_ids(orchestrator, &input.child_ids) {
                Ok(child_ids) => child_ids,
                Err(error) => return Task::ready(Err(error)),
            };
            let records = child_ids
                .iter()
                .filter_map(|child| this.child(orchestrator, *child))
                .collect::<Vec<_>>();
            let receivers = records
                .iter()
                .map(|record| record.subscribe_status())
                .collect::<Vec<_>>();
            let urgent_receivers = records
                .iter()
                .map(|record| record.urgent_sender.receiver())
                .collect::<Vec<_>>();
            let urgent_waiting = records.iter().any(|record| record.has_undelivered_urgent());
            (child_ids, receivers, urgent_receivers, urgent_waiting)
        };
        let timeout = Duration::from_secs(
            input
                .timeout_secs
                .unwrap_or(DEFAULT_AWAIT_TIMEOUT_SECS)
                .min(MAX_AWAIT_TIMEOUT_SECS),
        );
        let timer = cx.background_executor().timer(timeout);
        let registry_handle = registry.clone();
        let registry = registry.downgrade();

        cx.spawn(async move |cx| {
            let mut timed_out = false;
            if !urgent_waiting {
                let all_finished =
                    future::join_all(receivers.into_iter().map(wait_until_terminal)).map(|_| ());
                let urgent_or_timeout = async {
                    match future::select(
                        Box::pin(wait_for_urgent(urgent_receivers)),
                        Box::pin(timer),
                    )
                    .await
                    {
                        future::Either::Left(_) => false,
                        future::Either::Right(_) => true,
                    }
                };
                timed_out =
                    match future::select(Box::pin(all_finished), Box::pin(urgent_or_timeout)).await
                    {
                        future::Either::Left(_) => false,
                        future::Either::Right((timed_out, _)) => timed_out,
                    };
            }
            let summaries = cx.update(|cx| {
                Self::collect_human_interventions(&registry_handle, orchestrator, &child_ids, cx)
            });
            summaries.await;
            registry.update(cx, |registry, _| {
                let mut children = Vec::new();
                for child in &child_ids {
                    let Some(record) = registry.child_mut(orchestrator, *child) else {
                        continue;
                    };
                    children.push(ChildResult {
                        child_id: record.child_id.to_key_string(),
                        title: record.title.to_string(),
                        status: record.status,
                        final_message: record
                            .status
                            .is_terminal()
                            .then(|| record.final_message.clone())
                            .flatten(),
                        pending_messages: record.take_pending_messages(),
                        human_interventions: std::mem::take(&mut record.interventions),
                    });
                }
                let timed_out =
                    timed_out && children.iter().any(|child| !child.status.is_terminal());
                AwaitChildrenOutput {
                    children,
                    timed_out,
                }
            })
        })
    }
}

impl ChildRecord {
    fn apply_status(&mut self, status: ChildStatus) {
        self.status = status;
        self.last_activity = Utc::now();
        // `send` reports only that nobody is subscribed right now, and the
        // value is stored either way, so later subscribers still see it.
        self.status_sender.send(status).ok();
    }
}

/// Completes when any of the children sends an urgent message. Never
/// completes when there is nobody to wait on.
async fn wait_for_urgent(receivers: Vec<watch::Receiver<u32>>) {
    let waits = receivers.into_iter().map(|mut receiver| {
        Box::pin(async move {
            // A dropped sender means the child was forgotten, not a message.
            if receiver.changed().await.is_err() {
                future::pending::<()>().await;
            }
        })
    });
    if waits.len() == 0 {
        return future::pending().await;
    }
    future::select_all(waits).await;
}

async fn wait_until_terminal(mut receiver: watch::Receiver<ChildStatus>) {
    loop {
        if receiver.borrow().is_terminal() {
            return;
        }
        // A dropped sender means the child was forgotten: stop waiting.
        if receiver.changed().await.is_err() {
            return;
        }
    }
}

fn log_exchange(log: &ChildLog, direction: LogDirection, text: &str, status: ChildStatus) {
    if let Err(error) = log.append(direction, text, status) {
        log::error!(
            "failed to write orchestration log {}: {error:#}",
            log.path().display()
        );
    }
}

fn same_host(a: &Rc<dyn OrchestrationHost>, b: &Rc<dyn OrchestrationHost>) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use gpui::BorrowAppContext as _;
    use gpui::TestAppContext;

    fn new_child(log_dir: &Path, title: &str) -> NewChild {
        NewChild {
            child_id: ThreadId::new(),
            title: title.to_string().into(),
            agent_id: "zed-agent".to_string(),
            model: None,
            log_dir: log_dir.to_path_buf(),
            prompt: format!("prompt for {title}"),
            orchestrator_title: "orchestrator".to_string(),
        }
    }

    fn register(
        registry: &Entity<OrchestrationRegistry>,
        orchestrator: ThreadId,
        log_dir: &Path,
        title: &str,
        cx: &mut TestAppContext,
    ) -> ThreadId {
        let child = new_child(log_dir, title);
        let child_id = child.child_id;
        registry.update(cx, |registry, cx| {
            registry.register_child(orchestrator, child, cx);
        });
        child_id
    }

    fn init_registry(cx: &mut TestAppContext) -> Entity<OrchestrationRegistry> {
        cx.update(OrchestrationRegistry::global_or_init)
    }

    #[gpui::test]
    fn test_status_state_machine(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);

        registry.read_with(cx, |registry, _| {
            let record = registry.child(orchestrator, child).unwrap();
            assert_eq!(record.status, ChildStatus::Starting);
            assert_eq!(record.turns, 0);
            assert_eq!(registry.orchestrator_of(child), Some(orchestrator));
        });

        registry.update(cx, |registry, cx| {
            registry.set_status(orchestrator, child, ChildStatus::Running, cx);
            registry.set_status(orchestrator, child, ChildStatus::WaitingForConfirmation, cx);
            registry.set_status(orchestrator, child, ChildStatus::Running, cx);
            registry.record_turn_finished(
                orchestrator,
                child,
                ChildStatus::Completed,
                Some("all done".into()),
                cx,
            );
        });
        registry.read_with(cx, |registry, _| {
            let record = registry.child(orchestrator, child).unwrap();
            assert_eq!(record.status, ChildStatus::Completed);
            assert_eq!(record.turns, 1);
            assert_eq!(record.final_message.as_deref(), Some("all done"));
        });

        // A follow-up reopens a finished child.
        registry.update(cx, |registry, cx| {
            registry.record_message_to_child(orchestrator, child, "one more thing", cx);
        });
        registry.read_with(cx, |registry, _| {
            let record = registry.child(orchestrator, child).unwrap();
            assert_eq!(record.status, ChildStatus::Running);
        });

        registry.update(cx, |registry, cx| {
            registry.record_turn_finished(
                orchestrator,
                child,
                ChildStatus::Failed,
                Some("boom".into()),
                cx,
            );
        });
        registry.read_with(cx, |registry, _| {
            let record = registry.child(orchestrator, child).unwrap();
            assert_eq!(record.status, ChildStatus::Failed);
            assert_eq!(record.turns, 2);
        });
    }

    #[gpui::test]
    fn test_children_are_scoped_to_their_orchestrator(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let first = ThreadId::new();
        let second = ThreadId::new();
        let child = register(&registry, first, directory.path(), "mine", cx);

        registry.read_with(cx, |registry, _| {
            assert_eq!(registry.children(first).len(), 1);
            assert!(registry.children(second).is_empty());
            assert!(
                registry
                    .resolve_child_id(second, &child.to_key_string())
                    .is_err()
            );
            assert!(registry.resolve_child_id(first, "not-a-uuid").is_err());
            assert_eq!(
                registry
                    .resolve_child_ids(first, &[child.to_key_string()])
                    .unwrap(),
                vec![child]
            );
        });
    }

    #[gpui::test]
    fn test_tokens_identify_the_orchestrator(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let first = ThreadId::new();
        let second = ThreadId::new();
        registry.update(cx, |registry, _| {
            let first_token = registry.mint_token(first);
            let second_token = registry.mint_token(second);
            assert_ne!(first_token, second_token);
            assert_eq!(registry.mint_token(first), first_token);
            assert_eq!(registry.resolve_token(&first_token), Some(first));
            assert_eq!(registry.resolve_token(&second_token), Some(second));
            assert_eq!(registry.resolve_token("unknown"), None);

            registry.revoke_tokens(first);
            assert_eq!(registry.resolve_token(&first_token), None);
            assert_eq!(registry.resolve_token(&second_token), Some(second));
        });
    }

    #[gpui::test]
    async fn test_await_children_waits_for_parallel_children_and_returns_only_final_messages(
        cx: &mut TestAppContext,
    ) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "first", cx);
        let second = register(&registry, orchestrator, directory.path(), "second", cx);

        let task = cx.update(|cx| {
            OrchestrationRegistry::await_children(
                &registry,
                orchestrator,
                AwaitChildrenInput {
                    child_ids: vec![first.to_key_string(), second.to_key_string()],
                    timeout_secs: Some(60),
                },
                cx,
            )
        });

        registry.update(cx, |registry, cx| {
            registry.set_status(orchestrator, first, ChildStatus::Running, cx);
            registry.set_status(orchestrator, second, ChildStatus::Running, cx);
        });
        cx.run_until_parked();

        registry.update(cx, |registry, cx| {
            registry.record_turn_finished(
                orchestrator,
                second,
                ChildStatus::Completed,
                Some("second result".into()),
                cx,
            );
        });
        cx.run_until_parked();

        registry.update(cx, |registry, cx| {
            registry.record_turn_finished(
                orchestrator,
                first,
                ChildStatus::Completed,
                Some("first result".into()),
                cx,
            );
        });

        let output = task.await.unwrap();
        assert!(!output.timed_out);
        assert_eq!(output.children.len(), 2);
        assert_eq!(output.children[0].child_id, first.to_key_string());
        assert_eq!(
            output.children[0].final_message.as_deref(),
            Some("first result")
        );
        assert_eq!(output.children[1].child_id, second.to_key_string());
        assert_eq!(
            output.children[1].final_message.as_deref(),
            Some("second result")
        );
    }

    #[gpui::test]
    async fn test_await_children_times_out_without_content(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let finished = register(&registry, orchestrator, directory.path(), "finished", cx);
        let running = register(&registry, orchestrator, directory.path(), "running", cx);
        registry.update(cx, |registry, cx| {
            registry.record_turn_finished(
                orchestrator,
                finished,
                ChildStatus::Completed,
                Some("finished result".into()),
                cx,
            );
            registry.set_status(orchestrator, running, ChildStatus::Running, cx);
            // A message left over from before the follow-up must not leak out
            // while the child is running again.
            if let Some(record) = registry.child_mut(orchestrator, running) {
                record.final_message = Some("stale".into());
            }
        });

        let task = cx.update(|cx| {
            OrchestrationRegistry::await_children(
                &registry,
                orchestrator,
                AwaitChildrenInput {
                    child_ids: Vec::new(),
                    timeout_secs: Some(5),
                },
                cx,
            )
        });
        cx.executor().advance_clock(Duration::from_secs(6));
        cx.run_until_parked();

        let output = task.await.unwrap();
        assert!(output.timed_out);
        assert_eq!(output.children[0].status, ChildStatus::Completed);
        assert_eq!(
            output.children[0].final_message.as_deref(),
            Some("finished result")
        );
        assert_eq!(output.children[1].status, ChildStatus::Running);
        assert_eq!(output.children[1].final_message, None);
    }

    #[gpui::test]
    async fn test_await_children_rejects_unknown_children(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let orchestrator = ThreadId::new();
        let task = cx.update(|cx| {
            OrchestrationRegistry::await_children(
                &registry,
                orchestrator,
                AwaitChildrenInput {
                    child_ids: vec![ThreadId::new().to_key_string()],
                    timeout_secs: None,
                },
                cx,
            )
        });
        assert!(task.await.is_err());
    }

    #[gpui::test]
    async fn test_child_status_returns_metadata_only(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        registry.update(cx, |registry, cx| {
            registry.record_turn_finished(
                orchestrator,
                child,
                ChildStatus::Completed,
                Some("SECRET-RESULT".into()),
                cx,
            );
        });

        let output = cx
            .update(|cx| {
                OrchestrationRegistry::child_status_with_summaries(
                    &registry,
                    orchestrator,
                    ChildStatusInput::default(),
                    cx,
                )
            })
            .await
            .unwrap();
        assert_eq!(output.children.len(), 1);
        assert_eq!(output.children[0].status, ChildStatus::Completed);
        assert_eq!(output.children[0].turns, 1);
        let serialized = serde_json::to_string(&output).unwrap();
        assert!(!serialized.contains("SECRET-RESULT"));
        assert!(!serialized.contains("prompt for worker"));
    }

    #[gpui::test]
    fn test_log_contains_only_orchestrator_child_exchanges(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);

        registry.update(cx, |registry, cx| {
            // Status changes are not exchanges, and neither is anything the
            // child does internally.
            registry.set_status(orchestrator, child, ChildStatus::Running, cx);
            registry.set_status(orchestrator, child, ChildStatus::WaitingForConfirmation, cx);
            registry.set_status(orchestrator, child, ChildStatus::Running, cx);
            registry.record_turn_finished(
                orchestrator,
                child,
                ChildStatus::Completed,
                Some("final answer".into()),
                cx,
            );
            registry.record_message_to_child(orchestrator, child, "follow-up", cx);
        });

        let log_path = registry.read_with(cx, |registry, _| {
            registry
                .child(orchestrator, child)
                .unwrap()
                .log_path
                .clone()
        });
        assert_eq!(
            log_path,
            directory
                .path()
                .join(orchestrator.to_key_string())
                .join(format!("{}.jsonl", child.to_key_string()))
        );
        let entries = ChildLog::new(directory.path(), orchestrator, child)
            .read()
            .unwrap();
        let lines = entries
            .iter()
            .map(|entry| (entry.direction, entry.text.as_str(), entry.status))
            .collect::<Vec<_>>();
        assert_eq!(
            lines,
            vec![
                (
                    LogDirection::ToChild,
                    "prompt for worker",
                    ChildStatus::Starting
                ),
                (
                    LogDirection::FromChild,
                    "final answer",
                    ChildStatus::Completed
                ),
                (LogDirection::ToChild, "follow-up", ChildStatus::Running),
            ]
        );
    }

    #[gpui::test]
    fn test_remove_orchestrator_drops_children_and_tokens(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        register(&registry, orchestrator, directory.path(), "worker", cx);
        let token = registry.update(cx, |registry, _| registry.mint_token(orchestrator));

        registry.update(cx, |registry, cx| {
            registry.remove_orchestrator(orchestrator, cx);
        });
        registry.read_with(cx, |registry, _| {
            assert!(registry.children(orchestrator).is_empty());
            assert_eq!(registry.resolve_token(&token), None);
        });
    }

    use crate::{
        AvailableAgents, CancelChildInput, CancelChildOutput, SendToChildInput, SendToChildOutput,
        StartChildOutput, child_header, human_summary_prompt,
    };
    use std::cell::{Cell, RefCell};

    /// A host that owns some threads, reports whether they are idle, and
    /// records what it is asked to deliver and to summarize.
    #[derive(Default)]
    struct TestHost {
        owned: RefCell<Vec<ThreadId>>,
        idle: Cell<bool>,
        delivered: RefCell<Vec<(ThreadId, String)>>,
        summary_requests: RefCell<Vec<HumanSummaryRequest>>,
        summary: RefCell<Option<String>>,
    }

    impl OrchestrationHost for TestHost {
        fn start_child(
            &self,
            _: ThreadId,
            _: crate::StartChildInput,
            _: &mut App,
        ) -> Task<Result<StartChildOutput>> {
            Task::ready(Err(anyhow!("unused")))
        }

        fn send_to_child(
            &self,
            _: ThreadId,
            _: SendToChildInput,
            _: &mut App,
        ) -> Task<Result<SendToChildOutput>> {
            Task::ready(Err(anyhow!("unused")))
        }

        fn cancel_child(
            &self,
            _: ThreadId,
            _: CancelChildInput,
            _: &mut App,
        ) -> Task<Result<CancelChildOutput>> {
            Task::ready(Err(anyhow!("unused")))
        }

        fn list_available_agents(&self, _: &mut App) -> Result<AvailableAgents> {
            Ok(AvailableAgents::default())
        }

        fn resolve_orchestrator(&self, _: &str, _: &App) -> Option<ThreadId> {
            None
        }

        fn owns_thread(&self, thread_id: ThreadId, _: &App) -> bool {
            self.owned.borrow().contains(&thread_id)
        }

        fn thread_title(&self, _: ThreadId, _: &App) -> Option<String> {
            Some("Live orchestrator title".to_string())
        }

        fn is_thread_idle(&self, _: ThreadId, _: &App) -> bool {
            self.idle.get()
        }

        fn deliver_message(&self, thread: ThreadId, text: String, _: &mut App) -> Task<Result<()>> {
            self.delivered.borrow_mut().push((thread, text));
            Task::ready(Ok(()))
        }

        fn summarize_human_interventions(
            &self,
            request: HumanSummaryRequest,
            _: &mut App,
        ) -> Task<Result<Option<String>>> {
            self.summary_requests.borrow_mut().push(request);
            Task::ready(Ok(self.summary.borrow().clone()))
        }
    }

    fn setup_host(
        registry: &Entity<OrchestrationRegistry>,
        threads: &[ThreadId],
        cx: &mut TestAppContext,
    ) -> Rc<TestHost> {
        let host = Rc::new(TestHost::default());
        host.owned.borrow_mut().extend_from_slice(threads);
        registry.update(cx, |registry, _| registry.register_host(host.clone()));
        host
    }

    async fn status_of(
        registry: &Entity<OrchestrationRegistry>,
        orchestrator: ThreadId,
        cx: &mut TestAppContext,
    ) -> Vec<ChildStatusInfo> {
        let task = cx.update(|cx| {
            OrchestrationRegistry::child_status_with_summaries(
                registry,
                orchestrator,
                ChildStatusInput::default(),
                cx,
            )
        });
        task.await.unwrap().children
    }

    fn log_entries(
        directory: &Path,
        orchestrator: ThreadId,
        child: ThreadId,
    ) -> Vec<(LogDirection, String)> {
        ChildLog::new(directory, orchestrator, child)
            .read()
            .unwrap()
            .into_iter()
            .map(|entry| (entry.direction, entry.text))
            .collect()
    }

    #[test]
    fn test_child_header_names_the_orchestrator_and_the_tools() {
        let header = child_header("Refactor billing");
        assert!(header.contains("\"Refactor billing\""));
        assert!(header.contains("final message goes back to the orchestrator automatically"));
        assert!(header.contains("`send_to_parent`"));
        assert!(header.contains("`whoami`"));
    }

    #[test]
    fn test_truncate_summary_respects_the_cap() {
        assert_eq!(truncate_summary("  short  ", 600), "short");
        let long = "word ".repeat(400);
        let cut = truncate_summary(&long, 600);
        assert!(cut.chars().count() <= 600, "{}", cut.chars().count());
        assert!(cut.ends_with('\u{2026}'));
        // Multi-byte characters are counted as characters, not bytes.
        let accents = "\u{e9}".repeat(50);
        assert_eq!(truncate_summary(&accents, 10).chars().count(), 10);
        assert_eq!(truncate_summary("anything", 0), "");
        assert!(human_summary_prompt("t", 123).contains("123"));
    }

    #[gpui::test]
    fn test_whoami_roles(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let root = ThreadId::new();
        let middle = register(&registry, root, directory.path(), "middle", cx);
        let leaf = register(&registry, middle, directory.path(), "leaf", cx);
        let loner = ThreadId::new();
        setup_host(&registry, &[root, middle, leaf], cx);

        cx.update(|cx| {
            let registry = registry.read(cx);
            let whoami = registry.whoami(loner, cx);
            assert_eq!(whoami.role, ThreadRole::None);
            assert!(whoami.parent.is_none() && whoami.children.is_empty());

            let whoami = registry.whoami(root, cx);
            assert_eq!(whoami.role, ThreadRole::Orchestrator);
            assert_eq!(whoami.children.len(), 1);
            assert_eq!(whoami.children[0].id, middle.to_key_string());
            assert_eq!(whoami.children[0].title, "middle");
            assert_eq!(whoami.children[0].status, ChildStatus::Starting);

            // A thread that started children and was started itself is both.
            let whoami = registry.whoami(middle, cx);
            assert_eq!(whoami.role, ThreadRole::Both);
            let parent = whoami.parent.unwrap();
            assert_eq!(parent.id, root.to_key_string());
            assert_eq!(parent.title, "Live orchestrator title");
            assert_eq!(whoami.children[0].id, leaf.to_key_string());

            let whoami = registry.whoami(leaf, cx);
            assert_eq!(whoami.role, ThreadRole::Child);
            assert!(whoami.children.is_empty());
        });
    }

    #[gpui::test]
    fn test_whoami_falls_back_to_the_title_stored_when_the_child_started(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        // No host owns the orchestrator.
        cx.update(|cx| {
            let whoami = registry.read(cx).whoami(child, cx);
            assert_eq!(whoami.parent.unwrap().title, "orchestrator");
        });
    }

    #[gpui::test]
    async fn test_send_to_parent_logs_fills_the_inbox_and_is_delivered_once(
        cx: &mut TestAppContext,
    ) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        let host = setup_host(&registry, &[orchestrator, child], cx);

        let output = cx
            .update(|cx| {
                OrchestrationRegistry::send_to_parent(
                    &registry,
                    child,
                    SendToParentInput {
                        message: "which schema do I use?".into(),
                        urgent: false,
                    },
                    cx,
                )
            })
            .unwrap();
        assert_eq!(output.parent_id, orchestrator.to_key_string());
        assert_eq!(output.parent_title, "Live orchestrator title");
        assert!(!output.urgent);

        let entries = log_entries(directory.path(), orchestrator, child);
        assert_eq!(
            entries.last(),
            Some(&(LogDirection::ToParent, "which schema do I use?".to_string()))
        );
        // A message that is not urgent never reaches the orchestrator's thread.
        assert!(host.delivered.borrow().is_empty());

        let first = status_of(&registry, orchestrator, cx).await;
        assert_eq!(first[0].pending_messages.len(), 1);
        assert_eq!(
            first[0].pending_messages[0].message,
            "which schema do I use?"
        );
        assert!(!first[0].pending_messages[0].urgent);
        // Returned once, then marked delivered.
        let second = status_of(&registry, orchestrator, cx).await;
        assert!(second[0].pending_messages.is_empty());

        let task = cx.update(|cx| {
            OrchestrationRegistry::send_to_parent(
                &registry,
                child,
                SendToParentInput {
                    message: "  ".into(),
                    urgent: false,
                },
                cx,
            )
        });
        assert!(task.is_err());
    }

    #[gpui::test]
    fn test_send_to_parent_requires_an_orchestrator(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let error = cx
            .update(|cx| {
                OrchestrationRegistry::send_to_parent(
                    &registry,
                    ThreadId::new(),
                    SendToParentInput {
                        message: "hello".into(),
                        urgent: true,
                    },
                    cx,
                )
            })
            .unwrap_err();
        assert!(error.to_string().contains("no orchestrator"), "{error}");
    }

    #[gpui::test]
    async fn test_pending_messages_reach_await_children_once(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        setup_host(&registry, &[orchestrator, child], cx);
        registry.update(cx, |registry, cx| {
            registry.record_turn_finished(
                orchestrator,
                child,
                ChildStatus::Completed,
                Some("done".into()),
                cx,
            );
        });
        cx.update(|cx| {
            OrchestrationRegistry::send_to_parent(
                &registry,
                child,
                SendToParentInput {
                    message: "note".into(),
                    urgent: false,
                },
                cx,
            )
        })
        .unwrap();

        let await_once = |cx: &mut TestAppContext| {
            cx.update(|cx| {
                OrchestrationRegistry::await_children(
                    &registry,
                    orchestrator,
                    AwaitChildrenInput::default(),
                    cx,
                )
            })
        };
        let first = await_once(cx).await.unwrap();
        assert_eq!(first.children[0].final_message.as_deref(), Some("done"));
        assert_eq!(first.children[0].pending_messages.len(), 1);
        let second = await_once(cx).await.unwrap();
        assert!(second.children[0].pending_messages.is_empty());
    }

    #[gpui::test]
    async fn test_urgent_message_waits_for_an_idle_orchestrator(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        let host = setup_host(&registry, &[orchestrator, child], cx);
        let send = |text: &str, urgent: bool, cx: &mut TestAppContext| {
            cx.update(|cx| {
                OrchestrationRegistry::send_to_parent(
                    &registry,
                    child,
                    SendToParentInput {
                        message: text.into(),
                        urgent,
                    },
                    cx,
                )
            })
            .unwrap();
        };

        // The orchestrator is in the middle of a turn.
        host.idle.set(false);
        send("blocked: need the API key", true, cx);
        cx.run_until_parked();
        assert!(
            host.delivered.borrow().is_empty(),
            "a running turn must not receive anything"
        );

        // It stops: whoever watches it flushes, and the message arrives once.
        host.idle.set(true);
        cx.update(|cx| OrchestrationRegistry::flush_urgent(&registry, orchestrator, cx));
        cx.run_until_parked();
        assert_eq!(
            *host.delivered.borrow(),
            vec![(
                orchestrator,
                "[from child worker] blocked: need the API key".to_string()
            )]
        );
        cx.update(|cx| OrchestrationRegistry::flush_urgent(&registry, orchestrator, cx));
        assert_eq!(host.delivered.borrow().len(), 1);

        // What was delivered as a message is not returned a second time.
        assert!(
            status_of(&registry, orchestrator, cx).await[0]
                .pending_messages
                .is_empty()
        );

        // Idle orchestrator: an urgent message goes straight in.
        send("second blocker", true, cx);
        cx.run_until_parked();
        assert_eq!(host.delivered.borrow().len(), 2);
    }

    #[gpui::test]
    async fn test_urgent_message_read_through_a_tool_is_not_delivered_again(
        cx: &mut TestAppContext,
    ) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        let host = setup_host(&registry, &[orchestrator, child], cx);
        host.idle.set(false);
        cx.update(|cx| {
            OrchestrationRegistry::send_to_parent(
                &registry,
                child,
                SendToParentInput {
                    message: "need a decision".into(),
                    urgent: true,
                },
                cx,
            )
        })
        .unwrap();

        let statuses = status_of(&registry, orchestrator, cx).await;
        assert_eq!(statuses[0].pending_messages.len(), 1);
        assert!(statuses[0].pending_messages[0].urgent);

        host.idle.set(true);
        cx.update(|cx| OrchestrationRegistry::flush_urgent(&registry, orchestrator, cx));
        cx.run_until_parked();
        assert!(host.delivered.borrow().is_empty());
    }

    #[gpui::test]
    async fn test_urgent_message_wakes_await_children(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        let host = setup_host(&registry, &[orchestrator, child], cx);
        host.idle.set(false);
        registry.update(cx, |registry, cx| {
            registry.set_status(orchestrator, child, ChildStatus::Running, cx);
        });

        let task = cx.update(|cx| {
            OrchestrationRegistry::await_children(
                &registry,
                orchestrator,
                AwaitChildrenInput::default(),
                cx,
            )
        });
        cx.run_until_parked();
        cx.update(|cx| {
            OrchestrationRegistry::send_to_parent(
                &registry,
                child,
                SendToParentInput {
                    message: "blocked".into(),
                    urgent: true,
                },
                cx,
            )
        })
        .unwrap();

        let output = task.await.unwrap();
        assert!(!output.timed_out);
        assert_eq!(output.children[0].status, ChildStatus::Running);
        assert_eq!(output.children[0].pending_messages[0].message, "blocked");
        assert!(host.delivered.borrow().is_empty());
    }

    #[gpui::test]
    fn test_human_turns_are_neither_logged_nor_forwarded(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        let host = setup_host(&registry, &[orchestrator, child], cx);
        host.idle.set(true);
        let before = log_entries(directory.path(), orchestrator, child);

        registry.update(cx, |registry, _| {
            registry.note_human_turn(child, 4);
            registry.note_human_turn(child, 9);
            // A thread that is not a child is ignored.
            registry.note_human_turn(orchestrator, 0);
        });
        cx.run_until_parked();

        assert_eq!(log_entries(directory.path(), orchestrator, child), before);
        assert!(host.delivered.borrow().is_empty());
        assert!(host.summary_requests.borrow().is_empty());
        registry.read_with(cx, |registry, _| {
            assert_eq!(registry.pending_human_turns(child), 2);
            assert_eq!(registry.pending_human_turns(orchestrator), 0);
        });
    }

    #[gpui::test]
    async fn test_summary_is_generated_on_demand_and_covers_all_human_turns(
        cx: &mut TestAppContext,
    ) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        let host = setup_host(&registry, &[orchestrator, child], cx);
        *host.summary.borrow_mut() =
            Some("A human intervened and switched the child to tabs, to match the repo.".into());

        registry.update(cx, |registry, _| {
            // An orchestrator message before the first human turn is outside
            // the summary's range and is not tracked.
            registry.note_orchestrator_turn(child, 1);
            registry.note_human_turn(child, 3);
            registry.note_orchestrator_turn(child, 5);
            registry.note_human_turn(child, 6);
            registry.note_human_turn(child, 9);
        });
        // Nothing asked yet: no model call.
        cx.run_until_parked();
        assert!(host.summary_requests.borrow().is_empty());

        let statuses = status_of(&registry, orchestrator, cx).await;
        let requests = host.summary_requests.borrow();
        assert_eq!(requests.len(), 1, "three human turns make one summary");
        assert_eq!(requests[0].child, child);
        assert_eq!(requests[0].turns, 3);
        assert_eq!(requests[0].from_entry, 3);
        assert_eq!(requests[0].orchestrator_entries, vec![5]);
        assert_eq!(requests[0].max_chars, DEFAULT_HUMAN_SUMMARY_MAX_CHARS);

        let interventions = &statuses[0].human_interventions;
        assert_eq!(interventions.len(), 1);
        assert_eq!(interventions[0].turns, 3);
        assert!(
            interventions[0]
                .summary
                .contains("switched the child to tabs")
        );
        assert_eq!(
            log_entries(directory.path(), orchestrator, child).last(),
            Some(&(
                LogDirection::HumanIntervention,
                interventions[0].summary.clone()
            ))
        );
        drop(requests);

        // Returned once, and a later call does not summarize again.
        let again = status_of(&registry, orchestrator, cx).await;
        assert!(again[0].human_interventions.is_empty());
        assert_eq!(host.summary_requests.borrow().len(), 1);

        // The next summary starts after the previous one.
        registry.update(cx, |registry, _| registry.note_human_turn(child, 12));
        let next = status_of(&registry, orchestrator, cx).await;
        assert_eq!(next[0].human_interventions[0].turns, 1);
        assert_eq!(host.summary_requests.borrow()[1].from_entry, 12);
    }

    #[gpui::test]
    async fn test_summary_reaches_await_children(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        let host = setup_host(&registry, &[orchestrator, child], cx);
        *host.summary.borrow_mut() = Some("A human adjusted the plan.".into());
        registry.update(cx, |registry, cx| {
            registry.note_human_turn(child, 1);
            registry.record_turn_finished(
                orchestrator,
                child,
                ChildStatus::Completed,
                Some("done".into()),
                cx,
            );
        });
        let output = cx
            .update(|cx| {
                OrchestrationRegistry::await_children(
                    &registry,
                    orchestrator,
                    AwaitChildrenInput::default(),
                    cx,
                )
            })
            .await
            .unwrap();
        assert_eq!(
            output.children[0].human_interventions[0].summary,
            "A human adjusted the plan."
        );
    }

    #[gpui::test]
    async fn test_summary_is_capped_by_the_setting(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let store = SettingsStore::test(cx);
            cx.set_global(store);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |settings| {
                    settings
                        .agent
                        .get_or_insert_default()
                        .orchestration
                        .get_or_insert_default()
                        .human_summary_max_chars = Some(80);
                });
            });
            assert_eq!(human_summary_max_chars(cx), 80);
        });
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        let host = setup_host(&registry, &[orchestrator, child], cx);
        *host.summary.borrow_mut() = Some("The human changed the approach. ".repeat(40));
        registry.update(cx, |registry, _| registry.note_human_turn(child, 0));

        let statuses = status_of(&registry, orchestrator, cx).await;
        assert_eq!(host.summary_requests.borrow()[0].max_chars, 80);
        let summary = &statuses[0].human_interventions[0].summary;
        assert!(summary.chars().count() <= 80, "{}", summary.chars().count());
    }

    #[gpui::test]
    async fn test_summary_without_a_model_is_a_minimal_note(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register(&registry, orchestrator, directory.path(), "worker", cx);
        // The host has no summary model: it answers `None`.
        setup_host(&registry, &[orchestrator, child], cx);
        registry.update(cx, |registry, _| {
            registry.note_human_turn(child, 0);
            registry.note_human_turn(child, 2);
        });

        let statuses = status_of(&registry, orchestrator, cx).await;
        assert_eq!(
            statuses[0].human_interventions[0].summary,
            "A human intervened in 2 turns; no summary model configured."
        );
        assert_eq!(
            human_summary_fallback(1),
            "A human intervened in 1 turn; no summary model configured."
        );
    }
}
