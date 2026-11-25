use std::{path::PathBuf, rc::Rc, time::Duration};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use collections::HashMap;
use futures::{FutureExt as _, channel::oneshot, future};
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, Global, ReadGlobal as _, SharedString,
    Task,
};
use settings::SettingsStore;

use crate::{AnnouncedSubagent, SessionQuotaGrant, SubagentQuota, SubagentQuotaStatus};
use crate::{
    AwaitChildrenInput, AwaitChildrenOutput, ChildLog, ChildReference, ChildResult, ChildStatus,
    ChildStatusInfo, ChildStatusInput, ChildStatusOutput, DEFAULT_AWAIT_TIMEOUT_SECS,
    DEFAULT_MAX_CONCURRENT_CHILDREN, DEFAULT_MAX_SIBLING_MESSAGES,
    DEFAULT_SIBLING_REPLY_TIMEOUT_SECS, HumanIntervention, HumanSummaryRequest, ListSiblingsOutput,
    LogDirection, MAX_AWAIT_TIMEOUT_SECS, MessageSender, OrchestrationHost, PendingMessage,
    SendToParentInput, SendToParentOutput, SendToSiblingInput, SendToSiblingOutput, SenderRole,
    SiblingInfo, ThreadId, ThreadReference, ThreadRole, WhoAmIOutput, human_summary_fallback,
    log_dir_from_setting, sibling_message_text, truncate_summary, urgent_message_prefix,
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
    /// Sibling messages this child sent or received.
    sibling_messages: u32,
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

/// The `agent.orchestration.max_concurrent_children` setting.
pub fn max_concurrent_children(cx: &App) -> usize {
    orchestration_setting(cx, |orchestration| orchestration.max_concurrent_children)
        .unwrap_or(DEFAULT_MAX_CONCURRENT_CHILDREN)
}

/// The `agent.orchestration.max_sibling_messages` setting.
pub fn max_sibling_messages(cx: &App) -> usize {
    orchestration_setting(cx, |orchestration| orchestration.max_sibling_messages)
        .unwrap_or(DEFAULT_MAX_SIBLING_MESSAGES)
}

fn orchestration_setting(
    cx: &App,
    pick: impl FnOnce(&settings::OrchestrationSettingsContent) -> Option<usize>,
) -> Option<usize> {
    cx.try_global::<SettingsStore>().and_then(|store| {
        store
            .merged_settings()
            .agent
            .as_ref()
            .and_then(|agent| agent.orchestration.as_ref())
            .and_then(pick)
    })
}

/// A child to rebuild in the registry after a restart, from its persisted
/// thread metadata. The child is not loaded: nothing observes it until
/// something opens it.
pub struct RestoredChild {
    pub child_id: ThreadId,
    pub title: SharedString,
    pub agent_id: String,
    pub model: Option<String>,
    pub log_dir: PathBuf,
    pub orchestrator_title: String,
}

/// A message from one child to a sibling, waiting for the sibling to be idle.
struct SiblingMessage {
    id: u64,
    from: ThreadId,
    from_title: String,
    to: ThreadId,
    text: String,
    /// The sender is blocked in `send_to_sibling` until the receiver answers.
    awaiting_reply: bool,
    delivered: bool,
}

/// A child blocked in `send_to_sibling` with `await_reply`. The reply is the
/// next message `target` sends to `waiter`.
struct SiblingWait {
    waiter: ThreadId,
    target: ThreadId,
    reply: oneshot::Sender<String>,
}

impl SiblingWait {
    /// A wait whose receiver was dropped (the tool call was cancelled) does
    /// not block anyone.
    fn is_live(&self) -> bool {
        !self.reply.is_canceled()
    }
}

struct SentSibling {
    orchestrator: ThreadId,
    target: ThreadId,
    target_title: String,
    /// The message answered a wait of the receiver, so it needs no delivery.
    consumed_as_reply: bool,
    reply: Option<oneshot::Receiver<String>>,
    messages_left: u32,
}

fn pair_key(a: ThreadId, b: ThreadId) -> (uuid::Uuid, uuid::Uuid) {
    let (a, b) = (a.as_uuid(), b.as_uuid());
    if a < b { (a, b) } else { (b, a) }
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
    /// Sibling messages not yet delivered to their receiver's thread.
    sibling_inbox: Vec<SiblingMessage>,
    /// Messages exchanged by each pair of siblings, in both directions.
    pair_counts: HashMap<(uuid::Uuid, uuid::Uuid), u32>,
    sibling_waits: Vec<SiblingWait>,
    /// `start_child` calls that passed the concurrency cap and have not
    /// registered their child yet.
    pending_child_starts: usize,
    /// The subagent quota of each orchestrated child (R6).
    quotas: HashMap<ThreadId, SubagentQuota>,
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
        let quota = self
            .quotas
            .entry(child.child_id)
            .or_insert_with(|| SubagentQuota::new(0, None));
        log_exchange(
            &log,
            LogDirection::Quota,
            &format!("subagent_quota granted: {}", quota.granted()),
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
                sibling_messages: 0,
            });
        cx.emit(OrchestrationRegistryEvent::ChildChanged {
            orchestrator,
            child: child_id,
        });
        cx.notify();
        log_path
    }

    /// Rebuilds the record of a child that existed before a restart. Returns
    /// false, and changes nothing, when the registry already knows the child.
    ///
    /// Nothing runs after a restart, so the child is `Completed` unless its
    /// log says otherwise: its last logged status when that is `Failed` or
    /// `Cancelled`, and `Failed` when it ended with a message nobody answered.
    /// The last final message, the turn count and the sibling counters come
    /// from the child's log; without a log (cleared with the temporary
    /// directory) the child has no final message.
    pub fn restore_child(
        &mut self,
        orchestrator: ThreadId,
        child: RestoredChild,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.child(orchestrator, child.child_id).is_some() {
            return false;
        }
        let log = ChildLog::new(&child.log_dir, orchestrator, child.child_id);
        let entries = match log.read() {
            Ok(entries) => entries,
            Err(error) => {
                log::error!(
                    "failed to read orchestration log {}: {error:#}",
                    log.path().display()
                );
                Vec::new()
            }
        };
        let turns = entries
            .iter()
            .filter(|entry| entry.direction == LogDirection::FromChild)
            .count() as u32;
        let last_final = entries
            .iter()
            .rev()
            .find(|entry| entry.direction == LogDirection::FromChild);
        let last_exchange = entries.iter().rev().find(|entry| {
            matches!(
                entry.direction,
                LogDirection::ToChild | LogDirection::FromChild
            )
        });
        let (status, final_message) = match last_exchange {
            Some(entry) if entry.direction == LogDirection::ToChild => (
                ChildStatus::Failed,
                Some(
                    "Zed was restarted before this child answered its latest message.".to_string(),
                ),
            ),
            Some(entry) => (
                if entry.status.is_terminal() {
                    entry.status
                } else {
                    ChildStatus::Completed
                },
                last_final.map(|entry| entry.text.clone()),
            ),
            None => (ChildStatus::Completed, None),
        };
        let mut sibling_messages = 0;
        let mut per_sibling: HashMap<ThreadId, u32> = HashMap::default();
        for entry in &entries {
            if matches!(
                entry.direction,
                LogDirection::ToSibling | LogDirection::FromSibling
            ) {
                sibling_messages += 1;
                if let Some(other) = entry.sibling.as_deref().and_then(ThreadId::from_key_string) {
                    *per_sibling.entry(other).or_default() += 1;
                }
            }
        }
        for (other, count) in per_sibling {
            let slot = self
                .pair_counts
                .entry(pair_key(child.child_id, other))
                .or_default();
            *slot = (*slot).max(count);
        }
        let granted = entries
            .iter()
            .filter(|entry| entry.direction == LogDirection::Quota)
            .find_map(|entry| entry.text.strip_prefix("subagent_quota granted: "))
            .and_then(|count| count.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let used = entries
            .iter()
            .filter(|entry| entry.direction == LogDirection::Quota)
            .filter(|entry| entry.text.starts_with("subagent_quota used"))
            .count() as u32;
        self.quotas.insert(
            child.child_id,
            SubagentQuota::restored(granted, used.min(granted), None),
        );
        let (status_sender, _) = watch::channel(status);
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
                status,
                final_message,
                turns,
                last_activity: entries
                    .last()
                    .map(|entry| entry.ts)
                    .unwrap_or_else(Utc::now),
                log_path: log.path().to_path_buf(),
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
                sibling_messages,
            });
        cx.emit(OrchestrationRegistryEvent::ChildChanged {
            orchestrator,
            child: child_id,
        });
        cx.notify();
        true
    }

    /// Sets the subagent quota of `thread` before it is registered as a child:
    /// its ACP session may start before the registration, and the session
    /// options depend on the grant. `tickets_dir` is where Claude Code's hook
    /// finds its tickets.
    pub fn set_subagent_quota(&mut self, thread: ThreadId, granted: u32, tickets_dir: PathBuf) {
        self.quotas
            .insert(thread, SubagentQuota::new(granted, Some(tickets_dir)));
    }

    pub fn subagent_quota(&self, thread: ThreadId) -> Option<&SubagentQuota> {
        self.quotas.get(&thread)
    }

    /// What the ACP session of `thread` needs to enforce its quota.
    pub fn session_quota_grant(&self, thread: ThreadId) -> Option<SessionQuotaGrant> {
        let quota = self.quotas.get(&thread)?;
        Some(SessionQuotaGrant {
            granted: quota.granted(),
            tickets_dir: quota.tickets_dir()?.to_path_buf(),
        })
    }

    /// L1. Counts a subagent that `thread` starts through one of Zed's tools,
    /// or refuses it when the quota is used up. A thread that is not an
    /// orchestrated child has no quota.
    pub fn consume_subagent_quota(&mut self, thread: ThreadId, tool: &str) -> Result<()> {
        let Some(quota) = self.quotas.get_mut(&thread) else {
            return Ok(());
        };
        match quota.consume(tool) {
            Ok(()) => {
                let text = format!(
                    "subagent_quota used: {} of {} (`{tool}`)",
                    quota.used(),
                    quota.granted()
                );
                self.log_quota_event(thread, &text);
                Ok(())
            }
            Err(error) => {
                let text = format!("subagent_quota refused: `{tool}`");
                self.log_quota_event(thread, &text);
                Err(error)
            }
        }
    }

    /// Gives back a spawn that [`Self::consume_subagent_quota`] counted and
    /// that then failed.
    pub fn refund_subagent_quota(&mut self, thread: ThreadId) {
        if let Some(quota) = self.quotas.get_mut(&thread) {
            quota.refund();
        }
    }

    /// L4. Records that the agent running `thread` announced the subagent
    /// `subagent_session`, and returns whether Zed must cancel it now: it is
    /// over the quota, running, and the agent advertised `capabilities.cancel`. Either
    /// way, an extra subagent is logged in the child's R4 log.
    pub fn check_announced_subagent(
        &mut self,
        thread: ThreadId,
        subagent_session: &str,
        can_cancel: bool,
        live: bool,
    ) -> bool {
        let Some(quota) = self.quotas.get_mut(&thread) else {
            return false;
        };
        let granted = quota.granted();
        match quota.note_announced(subagent_session, can_cancel, live) {
            AnnouncedSubagent::Allow => false,
            AnnouncedSubagent::Cancel => {
                let text = format!(
                    "subagent_quota exceeded: cancelled subagent {subagent_session} \
                     (quota {granted})"
                );
                self.log_quota_event(thread, &text);
                true
            }
            AnnouncedSubagent::OverQuotaNotCancellable => {
                let text = format!(
                    "subagent_quota exceeded: subagent {subagent_session} is over the quota of \
                     {granted}, but the agent does not allow cancelling it"
                );
                self.log_quota_event(thread, &text);
                false
            }
        }
    }

    fn log_quota_event(&self, thread: ThreadId, text: &str) {
        if let Some(record) = self
            .children
            .values()
            .flatten()
            .find(|record| record.child_id == thread)
        {
            log_exchange(&record.log, LogDirection::Quota, text, record.status);
        }
    }

    /// Children that have not finished their turn, across every orchestrator.
    pub fn running_children(&self) -> usize {
        self.children
            .values()
            .flatten()
            .filter(|record| !record.status.is_terminal())
            .count()
    }

    /// Takes a slot under `agent.orchestration.max_concurrent_children`, or
    /// fails with an error that states the count. Call
    /// [`Self::release_child_slot`] once the child is registered, or when
    /// starting it failed, so that concurrent `start_child` calls cannot all
    /// pass the cap together.
    pub fn reserve_child_slot(&mut self, limit: usize) -> Result<()> {
        let running = self.running_children() + self.pending_child_starts;
        if running >= limit {
            return Err(anyhow!(
                "Cannot start another child: {running} of {limit} children are already running \
                 (`agent.orchestration.max_concurrent_children`). Wait for one to finish with \
                 `await_children`, or cancel one with `cancel_child`, then try again."
            ));
        }
        self.pending_child_starts += 1;
        Ok(())
    }

    pub fn release_child_slot(&mut self) {
        self.pending_child_starts = self.pending_child_starts.saturating_sub(1);
    }

    /// Forgets a thread that is gone: its children, its tokens, its record in
    /// its own orchestrator's list, and its sibling messages and waits. A
    /// sibling blocked in `send_to_sibling` on it stops waiting. The logs stay
    /// on disk.
    pub fn remove_orchestrator(&mut self, orchestrator: ThreadId, cx: &mut Context<Self>) {
        self.revoke_tokens(orchestrator);
        let removed_children = self.children.remove(&orchestrator);
        let mut changed = removed_children.is_some();
        for record in removed_children.iter().flatten() {
            self.quotas.remove(&record.child_id);
        }
        self.quotas.remove(&orchestrator);
        for records in self.children.values_mut() {
            let before = records.len();
            records.retain(|record| record.child_id != orchestrator);
            changed |= records.len() != before;
        }
        self.sibling_inbox
            .retain(|message| message.from != orchestrator && message.to != orchestrator);
        self.sibling_waits
            .retain(|wait| wait.waiter != orchestrator && wait.target != orchestrator);
        let removed = orchestrator.as_uuid();
        self.pair_counts
            .retain(|(first, second), _| *first != removed && *second != removed);
        if changed {
            cx.notify();
        }
    }

    /// Revokes the tokens handed to the ACP session `session_id`, so its MCP
    /// connection can no longer call the orchestration tools. Call it when the
    /// session's thread is deleted.
    pub fn revoke_session_tokens(&mut self, session_id: &str) {
        self.session_tokens
            .retain(|_, session| session.as_deref() != Some(session_id));
    }

    /// Whether a token minted for an ACP session is still valid.
    pub fn is_session_token_live(&self, token: &str) -> bool {
        self.session_tokens.contains_key(token)
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

    /// The other children of the orchestrator that started `caller`.
    pub fn list_siblings(&self, caller: ThreadId) -> Result<ListSiblingsOutput> {
        let orchestrator = self
            .orchestrator_of(caller)
            .ok_or_else(no_orchestrator_error)?;
        Ok(ListSiblingsOutput {
            siblings: self
                .children(orchestrator)
                .iter()
                .filter(|record| record.child_id != caller)
                .map(|record| SiblingInfo {
                    child_id: record.child_id.to_key_string(),
                    title: record.title.to_string(),
                    agent: record.agent_id.clone(),
                    model: record.model.clone(),
                    status: record.status,
                })
                .collect(),
        })
    }

    /// `send_to_sibling`: logs the message in both children's logs, counts it,
    /// and puts it in the receiver's inbox. The receiver gets it as a new
    /// message in its thread once it is idle, and never in the middle of a
    /// turn. With `await_reply`, the task finishes with the sibling's next
    /// `send_to_sibling` to the caller, or when the timeout expires.
    pub fn send_to_sibling(
        registry: &Entity<Self>,
        caller: ThreadId,
        input: SendToSiblingInput,
        cx: &mut App,
    ) -> Task<Result<SendToSiblingOutput>> {
        let limit = max_sibling_messages(cx);
        Self::send_to_sibling_with_limit(registry, caller, input, limit, cx)
    }

    fn send_to_sibling_with_limit(
        registry: &Entity<Self>,
        caller: ThreadId,
        input: SendToSiblingInput,
        limit: usize,
        cx: &mut App,
    ) -> Task<Result<SendToSiblingOutput>> {
        let message = input.message.trim().to_string();
        if message.is_empty() {
            return Task::ready(Err(anyhow!("`message` is empty.")));
        }
        let sent = registry.update(cx, |registry, cx| {
            registry.record_sibling_message(
                caller,
                &input.child_id,
                &message,
                input.await_reply,
                limit,
                cx,
            )
        });
        let sent = match sent {
            Ok(sent) => sent,
            Err(error) => return Task::ready(Err(error)),
        };
        if !sent.consumed_as_reply {
            Self::deliver_sibling_messages(registry, sent.orchestrator, sent.target, cx);
        }
        let output = SendToSiblingOutput {
            child_id: sent.target.to_key_string(),
            title: sent.target_title,
            reply: None,
            timed_out: false,
            messages_left: sent.messages_left,
        };
        let Some(reply) = sent.reply else {
            return Task::ready(Ok(output));
        };
        let timeout = Duration::from_secs(
            input
                .timeout_secs
                .unwrap_or(DEFAULT_SIBLING_REPLY_TIMEOUT_SECS)
                .min(MAX_AWAIT_TIMEOUT_SECS),
        );
        let timer = cx.background_executor().timer(timeout);
        let registry = registry.downgrade();
        let target = sent.target;
        cx.spawn(async move |cx| {
            let outcome = future::select(reply, Box::pin(timer)).await;
            match outcome {
                future::Either::Left((Ok(reply), _)) => Ok(SendToSiblingOutput {
                    reply: Some(reply),
                    ..output
                }),
                future::Either::Left((Err(_), _)) => Err(anyhow!(
                    "The wait for {}'s reply ended because that thread or the orchestrator \
                     was closed.",
                    output.title
                )),
                future::Either::Right(_) => {
                    registry
                        .update(cx, |registry, _| {
                            registry
                                .sibling_waits
                                .retain(|wait| !(wait.waiter == caller && wait.target == target));
                        })
                        .ok();
                    Ok(SendToSiblingOutput {
                        timed_out: true,
                        ..output
                    })
                }
            }
        })
    }

    fn record_sibling_message(
        &mut self,
        caller: ThreadId,
        raw_target: &str,
        message: &str,
        await_reply: bool,
        limit: usize,
        cx: &mut Context<Self>,
    ) -> Result<SentSibling> {
        let orchestrator = self
            .orchestrator_of(caller)
            .ok_or_else(no_orchestrator_error)?;
        let target = ThreadId::from_key_string(raw_target)
            .filter(|target| self.child(orchestrator, *target).is_some())
            .ok_or_else(|| {
                anyhow!(
                    "Unknown sibling {raw_target:?}: it is not another child of your \
                     orchestrator. Call `list_siblings` for the ids."
                )
            })?;
        if target == caller {
            return Err(anyhow!("You cannot send a message to yourself."));
        }
        let target_title = self
            .child(orchestrator, target)
            .map(|record| record.title.to_string())
            .unwrap_or_default();
        let caller_title = self
            .child(orchestrator, caller)
            .map(|record| record.title.to_string())
            .unwrap_or_default();

        let key = pair_key(caller, target);
        let used = self.pair_counts.get(&key).copied().unwrap_or(0);
        if used as usize >= limit {
            return Err(anyhow!(
                "Sibling message limit reached: you and {target_title} already exchanged {used} \
                 of {limit} messages (`agent.orchestration.max_sibling_messages`). If you still \
                 need something, tell the orchestrator with `send_to_parent`."
            ));
        }

        self.sibling_waits.retain(SiblingWait::is_live);
        if await_reply && self.would_deadlock(caller, target) {
            return Err(anyhow!(
                "Deadlock avoided: {target_title} is already waiting for a reply from you, \
                 directly or through other siblings. Answer it with `send_to_sibling` without \
                 `await_reply`: that message is its reply and unblocks it."
            ));
        }

        let status = |registry: &Self, thread: ThreadId| {
            registry
                .child(orchestrator, thread)
                .map_or(ChildStatus::Running, |record| record.status)
        };
        let (caller_status, target_status) = (status(self, caller), status(self, target));
        if let Some(record) = self.child_mut(orchestrator, caller) {
            log_sibling(
                &record.log,
                LogDirection::ToSibling,
                message,
                caller_status,
                target,
            );
            record.sibling_messages += 1;
        }
        if let Some(record) = self.child_mut(orchestrator, target) {
            log_sibling(
                &record.log,
                LogDirection::FromSibling,
                message,
                target_status,
                caller,
            );
            record.sibling_messages += 1;
        }
        let count = self.pair_counts.entry(key).or_default();
        *count += 1;
        let messages_left = (limit as u32).saturating_sub(*count);

        let consumed_as_reply = match self
            .sibling_waits
            .iter()
            .position(|wait| wait.waiter == target && wait.target == caller)
        {
            Some(position) => self
                .sibling_waits
                .remove(position)
                .reply
                .send(message.to_string())
                .is_ok(),
            None => false,
        };
        if !consumed_as_reply {
            let id = self.next_message_id;
            self.next_message_id += 1;
            self.sibling_inbox.push(SiblingMessage {
                id,
                from: caller,
                from_title: caller_title,
                to: target,
                text: message.to_string(),
                awaiting_reply: await_reply,
                delivered: false,
            });
        }
        let reply = await_reply.then(|| {
            let (sender, receiver) = oneshot::channel();
            self.sibling_waits.push(SiblingWait {
                waiter: caller,
                target,
                reply: sender,
            });
            receiver
        });
        for child in [caller, target] {
            cx.emit(OrchestrationRegistryEvent::ChildChanged {
                orchestrator,
                child,
            });
        }
        cx.notify();
        Ok(SentSibling {
            orchestrator,
            target,
            target_title,
            consumed_as_reply,
            reply,
            messages_left,
        })
    }

    /// Whether `caller` waiting for `target` would close a cycle of waits:
    /// `target` waits for `caller`, directly or through other siblings.
    fn would_deadlock(&self, caller: ThreadId, target: ThreadId) -> bool {
        let mut current = target;
        for _ in 0..=self.sibling_waits.len() {
            let Some(wait) = self
                .sibling_waits
                .iter()
                .find(|wait| wait.waiter == current)
            else {
                return false;
            };
            if wait.target == caller {
                return true;
            }
            current = wait.target;
        }
        false
    }

    /// Gets the pending sibling messages to `recipient`, opening it first when
    /// it is not loaded (after a restart).
    fn deliver_sibling_messages(
        registry: &Entity<Self>,
        orchestrator: ThreadId,
        recipient: ThreadId,
        cx: &mut App,
    ) {
        if registry.read(cx).host_for_thread(recipient, cx).is_some() {
            Self::flush_sibling_inbox(registry, recipient, cx);
            return;
        }
        let Some(host) = registry.read(cx).host_for_thread(orchestrator, cx) else {
            return;
        };
        let load = host.load_child(orchestrator, recipient, cx);
        let registry = registry.downgrade();
        cx.spawn(async move |cx| {
            if let Err(error) = load.await {
                log::error!("failed to open a sibling to deliver a message: {error:#}");
                return;
            }
            let Some(registry) = registry.upgrade() else {
                return;
            };
            cx.update(|cx| OrchestrationRegistry::flush_sibling_inbox(&registry, recipient, cx));
        })
        .detach();
    }

    /// Delivers the sibling messages that wait for `recipient` as one new
    /// message in its thread, but only when the thread is idle. A running turn
    /// is never interrupted: the host calls this again when the thread stops.
    pub fn flush_sibling_inbox(registry: &Entity<Self>, recipient: ThreadId, cx: &mut App) {
        let Some(host) = registry.read(cx).host_for_thread(recipient, cx) else {
            return;
        };
        if !host.is_thread_idle(recipient, cx) {
            return;
        }
        // One message per sender, so each shows as `From <sibling>` in the
        // receiving thread.
        let deliveries = registry.update(cx, |registry, _| {
            let mut deliveries: Vec<(ThreadId, Vec<String>, Vec<u64>)> = Vec::new();
            for message in registry
                .sibling_inbox
                .iter_mut()
                .filter(|message| message.to == recipient && !message.delivered)
            {
                message.delivered = true;
                let index = match deliveries
                    .iter()
                    .position(|(from, _, _)| *from == message.from)
                {
                    Some(index) => index,
                    None => {
                        deliveries.push((message.from, Vec::new(), Vec::new()));
                        deliveries.len() - 1
                    }
                };
                deliveries[index].2.push(message.id);
                deliveries[index].1.push(sibling_message_text(
                    &message.from_title,
                    &message.from.to_key_string(),
                    &message.text,
                    message.awaiting_reply,
                ));
            }
            deliveries
        });
        for (from, parts, delivered_ids) in deliveries {
            let delivery = host.deliver_message(
                recipient,
                MessageSender {
                    role: SenderRole::Sibling,
                    thread: from,
                },
                parts.join("\n\n"),
                cx,
            );
            let registry = registry.downgrade();
            cx.spawn(async move |cx| {
                let outcome = delivery.await;
                registry
                    .update(cx, |registry, _| match outcome {
                        Ok(()) => registry
                            .sibling_inbox
                            .retain(|message| !delivered_ids.contains(&message.id)),
                        Err(error) => {
                            log::error!("failed to deliver a sibling message: {error:#}");
                            for message in &mut registry.sibling_inbox {
                                if delivered_ids.contains(&message.id) {
                                    message.delivered = false;
                                }
                            }
                        }
                    })
                    .ok();
            })
            .detach();
        }
    }

    /// Cancels the running turns of every descendant of `thread`, the deepest
    /// first, and records them as `Cancelled`. `thread` itself is not touched.
    /// This is the cascade for closing a thread and for `cancel_child` on a
    /// child that orchestrates; cancelling an orchestrator's own turn does not
    /// come here, because its children run in parallel by design. Returns the
    /// children that were cancelled.
    pub fn cancel_running_descendants(
        registry: &Entity<Self>,
        thread: ThreadId,
        cx: &mut App,
    ) -> Vec<ThreadId> {
        let children = registry
            .read(cx)
            .children(thread)
            .iter()
            .map(|record| (record.child_id, !record.status.is_terminal()))
            .collect::<Vec<_>>();
        let mut cancelled = Vec::new();
        for (child, running) in children {
            cancelled.extend(Self::cancel_running_descendants(registry, child, cx));
            if !running {
                continue;
            }
            let hosts = registry.read(cx).hosts.clone();
            for host in hosts {
                let Some(final_message) = host.cancel_thread_turn(child, cx) else {
                    continue;
                };
                registry.update(cx, |registry, cx| {
                    registry.record_turn_finished(
                        thread,
                        child,
                        ChildStatus::Cancelled,
                        final_message,
                        cx,
                    );
                });
                cancelled.push(child);
                break;
            }
        }
        cancelled
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
        // One message per child, so each shows as `From <child>` in the
        // orchestrator's thread.
        let deliveries = registry.update(cx, |registry, _| {
            let mut deliveries: Vec<(ThreadId, String, Vec<u64>)> = Vec::new();
            for record in registry
                .children
                .get_mut(&orchestrator)
                .into_iter()
                .flatten()
            {
                let prefix = urgent_message_prefix(&record.title);
                let mut parts = Vec::new();
                let mut ids = Vec::new();
                for message in record
                    .inbox
                    .iter_mut()
                    .filter(|message| message.urgent && !message.delivered)
                {
                    message.delivered = true;
                    ids.push(message.id);
                    parts.push(format!("{prefix} {}", message.text));
                }
                if !ids.is_empty() {
                    deliveries.push((record.child_id, parts.join("\n\n"), ids));
                }
            }
            deliveries
        });
        for (child, text, delivered_ids) in deliveries {
            let delivery = host.deliver_message(
                orchestrator,
                MessageSender {
                    role: SenderRole::Child,
                    thread: child,
                },
                text,
                cx,
            );
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
            let quota = self
                .quotas
                .get(child)
                .map(|quota| SubagentQuotaStatus {
                    granted: quota.granted(),
                    used: quota.used(),
                })
                .unwrap_or_default();
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
                sibling_messages: record.sibling_messages,
                subagent_quota: quota,
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

fn no_orchestrator_error() -> anyhow::Error {
    anyhow!(
        "This thread was not started by an orchestrator, so it has no siblings. Call `whoami` \
         to see your role."
    )
}

fn log_sibling(
    log: &ChildLog,
    direction: LogDirection,
    text: &str,
    status: ChildStatus,
    sibling: ThreadId,
) {
    if let Err(error) = log.append_sibling(direction, text, status, sibling) {
        log::error!(
            "failed to write orchestration log {}: {error:#}",
            log.path().display()
        );
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
            .filter(|entry| entry.direction != LogDirection::Quota)
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
    use crate::{SendToSiblingInput, SendToSiblingOutput};
    use std::cell::{Cell, RefCell};

    /// A host that owns some threads, reports whether they are idle, and
    /// records what it is asked to deliver and to summarize.
    #[derive(Default)]
    struct TestHost {
        owned: RefCell<Vec<ThreadId>>,
        idle: Cell<bool>,
        delivered: RefCell<Vec<(ThreadId, String)>>,
        senders: RefCell<Vec<MessageSender>>,
        summary_requests: RefCell<Vec<HumanSummaryRequest>>,
        summary: RefCell<Option<String>>,
        /// Threads whose running turn this host can cancel.
        cancellable: RefCell<Vec<ThreadId>>,
        cancelled: RefCell<Vec<ThreadId>>,
        loaded: RefCell<Vec<ThreadId>>,
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

        fn deliver_message(
            &self,
            thread: ThreadId,
            from: MessageSender,
            text: String,
            _: &mut App,
        ) -> Task<Result<()>> {
            self.senders.borrow_mut().push(from);
            self.delivered.borrow_mut().push((thread, text));
            Task::ready(Ok(()))
        }

        fn cancel_thread_turn(&self, thread: ThreadId, _: &mut App) -> Option<Option<String>> {
            if !self.cancellable.borrow().contains(&thread) {
                return None;
            }
            self.cancelled.borrow_mut().push(thread);
            Some(Some(format!("partial work of {thread}")))
        }

        fn load_child(&self, _: ThreadId, child: ThreadId, _: &mut App) -> Task<Result<()>> {
            self.loaded.borrow_mut().push(child);
            self.owned.borrow_mut().push(child);
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
        let header = child_header("Refactor billing", 0);
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
        assert_eq!(
            *host.senders.borrow(),
            vec![MessageSender {
                role: SenderRole::Child,
                thread: child
            }]
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

    fn sibling_input(child: ThreadId, message: &str, await_reply: bool) -> SendToSiblingInput {
        SendToSiblingInput {
            child_id: child.to_key_string(),
            message: message.to_string(),
            await_reply,
            timeout_secs: Some(60),
        }
    }

    fn send_sibling(
        registry: &Entity<OrchestrationRegistry>,
        from: ThreadId,
        to: ThreadId,
        message: &str,
        await_reply: bool,
        cx: &mut TestAppContext,
    ) -> Task<Result<SendToSiblingOutput>> {
        cx.update(|cx| {
            OrchestrationRegistry::send_to_sibling(
                registry,
                from,
                sibling_input(to, message, await_reply),
                cx,
            )
        })
    }

    fn raw_log(
        directory: &Path,
        orchestrator: ThreadId,
        child: ThreadId,
    ) -> Vec<crate::ChildLogEntry> {
        ChildLog::new(directory, orchestrator, child)
            .read()
            .unwrap()
    }

    #[gpui::test]
    async fn test_sibling_message_is_delivered_and_logged_in_both_logs(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "api", cx);
        let second = register(&registry, orchestrator, directory.path(), "tests", cx);
        let host = setup_host(&registry, &[orchestrator, first, second], cx);
        host.idle.set(true);

        let output = send_sibling(&registry, first, second, "which file?", false, cx)
            .await
            .unwrap();
        cx.run_until_parked();
        assert_eq!(output.title, "tests");
        assert_eq!(output.messages_left, 19);
        assert_eq!(output.reply, None);

        // The receiver gets it as a new message with the sibling prefix, and
        // nobody else gets anything.
        let delivered = host.delivered.borrow();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].0, second);
        assert!(
            delivered[0].1.starts_with("[from sibling api] which file?"),
            "{}",
            delivered[0].1
        );
        assert!(delivered[0].1.contains(&first.to_key_string()));
        drop(delivered);
        assert_eq!(
            *host.senders.borrow(),
            vec![MessageSender {
                role: SenderRole::Sibling,
                thread: first
            }]
        );

        // Both children's logs hold the exchange, with the other child's id.
        let sent = raw_log(directory.path(), orchestrator, first);
        let sent = sent.last().unwrap();
        assert_eq!(sent.direction, LogDirection::ToSibling);
        assert_eq!(sent.text, "which file?");
        assert_eq!(sent.sibling, Some(second.to_key_string()));
        let received = raw_log(directory.path(), orchestrator, second);
        let received = received.last().unwrap();
        assert_eq!(received.direction, LogDirection::FromSibling);
        assert_eq!(received.sibling, Some(first.to_key_string()));

        // The orchestrator sees a count and no content.
        let statuses = status_of(&registry, orchestrator, cx).await;
        assert_eq!(statuses[0].sibling_messages, 1);
        assert_eq!(statuses[1].sibling_messages, 1);
        assert!(statuses.iter().all(|info| info.pending_messages.is_empty()));
        assert!(
            !serde_json::to_string(&statuses)
                .unwrap()
                .contains("which file?")
        );
    }

    #[gpui::test]
    async fn test_sibling_message_waits_for_an_idle_thread(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "a", cx);
        let second = register(&registry, orchestrator, directory.path(), "b", cx);
        let host = setup_host(&registry, &[orchestrator, first, second], cx);

        // The receiver is mid-turn: nothing is delivered, nothing interrupts it.
        send_sibling(&registry, first, second, "one", false, cx)
            .await
            .unwrap();
        send_sibling(&registry, first, second, "two", false, cx)
            .await
            .unwrap();
        cx.run_until_parked();
        assert!(host.delivered.borrow().is_empty());

        // When the turn ends, both arrive together, once.
        host.idle.set(true);
        cx.update(|cx| OrchestrationRegistry::flush_sibling_inbox(&registry, second, cx));
        cx.run_until_parked();
        {
            let delivered = host.delivered.borrow();
            assert_eq!(delivered.len(), 1);
            assert!(delivered[0].1.contains("one") && delivered[0].1.contains("two"));
        }
        cx.update(|cx| OrchestrationRegistry::flush_sibling_inbox(&registry, second, cx));
        cx.run_until_parked();
        assert_eq!(host.delivered.borrow().len(), 1);
    }

    #[gpui::test]
    async fn test_siblings_are_the_other_children_of_the_same_orchestrator(
        cx: &mut TestAppContext,
    ) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let other_orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "a", cx);
        let second = register(&registry, orchestrator, directory.path(), "b", cx);
        let stranger = register(&registry, other_orchestrator, directory.path(), "c", cx);

        registry.read_with(cx, |registry, _| {
            let siblings = registry.list_siblings(first).unwrap().siblings;
            assert_eq!(siblings.len(), 1);
            assert_eq!(siblings[0].child_id, second.to_key_string());
            // An orchestrator that is not a child has no siblings.
            assert!(registry.list_siblings(orchestrator).is_err());
        });
        let error = send_sibling(&registry, first, stranger, "hi", false, cx)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Unknown sibling"), "{error}");
        let error = send_sibling(&registry, first, first, "hi", false, cx)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("yourself"), "{error}");
        let error = send_sibling(&registry, orchestrator, first, "hi", false, cx)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("no siblings"), "{error}");
    }

    #[gpui::test]
    async fn test_sibling_messages_per_pair_are_capped(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "a", cx);
        let second = register(&registry, orchestrator, directory.path(), "b", cx);
        let third = register(&registry, orchestrator, directory.path(), "c", cx);

        let send = |from, to, cx: &mut TestAppContext| {
            cx.update(|cx| {
                OrchestrationRegistry::send_to_sibling_with_limit(
                    &registry,
                    from,
                    sibling_input(to, "msg", false),
                    2,
                    cx,
                )
            })
        };
        assert_eq!(send(first, second, cx).await.unwrap().messages_left, 1);
        // Both directions count toward the same pair.
        assert_eq!(send(second, first, cx).await.unwrap().messages_left, 0);
        let error = send(first, second, cx).await.unwrap_err();
        assert!(error.to_string().contains("limit reached"), "{error}");
        assert!(error.to_string().contains("2 of 2"), "{error}");
        assert!(error.to_string().contains("send_to_parent"), "{error}");
        // Other pairs are unaffected.
        assert!(send(first, third, cx).await.is_ok());
        // A refused message is neither logged nor counted.
        let log = raw_log(directory.path(), orchestrator, first);
        assert_eq!(
            log.iter()
                .filter(|entry| entry.direction == LogDirection::ToSibling)
                .count(),
            2
        );

        // The default comes from the setting, and is 20.
        assert_eq!(cx.update(|cx| max_sibling_messages(cx)), 20);
        assert_eq!(cx.update(|cx| max_concurrent_children(cx)), 6);
    }

    #[gpui::test]
    async fn test_await_reply_returns_the_siblings_next_message_to_the_caller(
        cx: &mut TestAppContext,
    ) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "asker", cx);
        let second = register(&registry, orchestrator, directory.path(), "answerer", cx);
        let host = setup_host(&registry, &[orchestrator, first, second], cx);
        host.idle.set(true);

        let waiting = send_sibling(&registry, first, second, "which file?", true, cx);
        cx.run_until_parked();
        // The question reached the sibling, and tells it how to answer.
        {
            let delivered = host.delivered.borrow();
            assert_eq!(delivered.len(), 1);
            assert!(delivered[0].1.contains("is waiting for your reply"));
        }

        let answer = send_sibling(&registry, second, first, "src/lib.rs", false, cx)
            .await
            .unwrap();
        assert_eq!(answer.reply, None);
        let output = waiting.await.unwrap();
        assert_eq!(output.reply.as_deref(), Some("src/lib.rs"));
        assert!(!output.timed_out);
        cx.run_until_parked();

        // The reply came back as the tool result, so it is not also added to
        // the asker's thread. Both messages are in both logs.
        assert_eq!(host.delivered.borrow().len(), 1);
        let asker_log = raw_log(directory.path(), orchestrator, first);
        let directions: Vec<_> = asker_log
            .iter()
            .map(|entry| entry.direction)
            .filter(|direction| {
                matches!(
                    direction,
                    LogDirection::ToSibling | LogDirection::FromSibling
                )
            })
            .collect();
        assert_eq!(
            directions,
            [LogDirection::ToSibling, LogDirection::FromSibling]
        );
    }

    #[gpui::test]
    async fn test_await_reply_times_out_and_stops_blocking(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "asker", cx);
        let second = register(&registry, orchestrator, directory.path(), "silent", cx);
        let host = setup_host(&registry, &[orchestrator, first, second], cx);
        host.idle.set(true);

        let waiting = send_sibling(&registry, first, second, "hello?", true, cx);
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(61));
        let output = waiting.await.unwrap();
        assert!(output.timed_out);
        assert_eq!(output.reply, None);

        // The wait is gone: a later message from the sibling is a plain
        // message, delivered to the asker's thread.
        send_sibling(&registry, second, first, "sorry, late", false, cx)
            .await
            .unwrap();
        cx.run_until_parked();
        let delivered = host.delivered.borrow();
        assert_eq!(delivered.last().unwrap().0, first);
        assert!(delivered.last().unwrap().1.contains("sorry, late"));
    }

    #[gpui::test]
    async fn test_mutual_await_reply_is_a_deadlock_for_the_second_waiter(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "a", cx);
        let second = register(&registry, orchestrator, directory.path(), "b", cx);
        let third = register(&registry, orchestrator, directory.path(), "c", cx);
        let host = setup_host(&registry, &[orchestrator, first, second, third], cx);
        host.idle.set(true);

        let first_waits = send_sibling(&registry, first, second, "q1", true, cx);
        cx.run_until_parked();

        // `second` also waits on `first`: it gets the error, nothing is sent.
        let log_before = raw_log(directory.path(), orchestrator, second).len();
        let error = send_sibling(&registry, second, first, "q2", true, cx)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Deadlock"), "{error}");
        assert_eq!(
            raw_log(directory.path(), orchestrator, second).len(),
            log_before
        );

        // A longer cycle is caught the same way: first -> second, second ->
        // third, third -> first.
        let second_waits = send_sibling(&registry, second, third, "q3", true, cx);
        cx.run_until_parked();
        let error = send_sibling(&registry, third, first, "q4", true, cx)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Deadlock"), "{error}");

        // The way out: answer without waiting. That unblocks the waiter.
        send_sibling(&registry, third, second, "a3", false, cx)
            .await
            .unwrap();
        assert_eq!(second_waits.await.unwrap().reply.as_deref(), Some("a3"));
        send_sibling(&registry, second, first, "a1", false, cx)
            .await
            .unwrap();
        assert_eq!(first_waits.await.unwrap().reply.as_deref(), Some("a1"));
    }

    #[gpui::test]
    async fn test_sibling_message_opens_a_thread_that_is_not_loaded(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "a", cx);
        let second = register(&registry, orchestrator, directory.path(), "b", cx);
        // `second` is not loaded, as after a restart.
        let host = setup_host(&registry, &[orchestrator, first], cx);
        host.idle.set(true);

        send_sibling(&registry, first, second, "are you there?", false, cx)
            .await
            .unwrap();
        cx.run_until_parked();
        assert_eq!(*host.loaded.borrow(), vec![second]);
        let delivered = host.delivered.borrow();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].0, second);
    }

    fn running_tree(
        registry: &Entity<OrchestrationRegistry>,
        directory: &Path,
        cx: &mut TestAppContext,
    ) -> (ThreadId, ThreadId, ThreadId, ThreadId, ThreadId) {
        // root -> a (running) -> a1 (running), a2 (completed); root -> b (running)
        let root = ThreadId::new();
        let a = register(registry, root, directory, "a", cx);
        let a1 = register(registry, a, directory, "a1", cx);
        let a2 = register(registry, a, directory, "a2", cx);
        let b = register(registry, root, directory, "b", cx);
        registry.update(cx, |registry, cx| {
            for (parent, child) in [(root, a), (a, a1), (root, b)] {
                registry.set_status(parent, child, ChildStatus::Running, cx);
            }
            registry.record_turn_finished(a, a2, ChildStatus::Completed, Some("done".into()), cx);
        });
        (root, a, a1, a2, b)
    }

    #[gpui::test]
    fn test_cascade_cancels_running_descendants_deepest_first(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let (root, a, a1, a2, b) = running_tree(&registry, directory.path(), cx);
        let host = setup_host(&registry, &[root, a, a1, a2, b], cx);
        host.cancellable.borrow_mut().extend([a, a1, a2, b]);

        // Closing, archiving or deleting `root` runs this.
        let cancelled =
            cx.update(|cx| OrchestrationRegistry::cancel_running_descendants(&registry, root, cx));
        assert_eq!(cancelled, vec![a1, a, b]);
        // The completed grandchild is left alone, and `root` is not touched.
        assert_eq!(*host.cancelled.borrow(), vec![a1, a, b]);
        registry.read_with(cx, |registry, _| {
            for (parent, child) in [(a, a1), (root, a), (root, b)] {
                let record = registry.child(parent, child).unwrap();
                assert_eq!(record.status, ChildStatus::Cancelled);
                assert!(
                    record
                        .final_message
                        .as_deref()
                        .unwrap()
                        .contains("partial work")
                );
            }
            assert_eq!(
                registry.child(a, a2).unwrap().status,
                ChildStatus::Completed
            );
        });

        // Nothing is running any more: a second cascade does nothing.
        let again =
            cx.update(|cx| OrchestrationRegistry::cancel_running_descendants(&registry, root, cx));
        assert!(again.is_empty());
    }

    #[gpui::test]
    fn test_cascade_from_a_child_that_orchestrates_leaves_its_siblings(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let (root, a, a1, a2, b) = running_tree(&registry, directory.path(), cx);
        let host = setup_host(&registry, &[root, a, a1, a2, b], cx);
        host.cancellable.borrow_mut().extend([a, a1, b]);

        // `cancel_child(a)` cascades to the children of `a` only.
        let cancelled =
            cx.update(|cx| OrchestrationRegistry::cancel_running_descendants(&registry, a, cx));
        assert_eq!(cancelled, vec![a1]);
        registry.read_with(cx, |registry, _| {
            assert_eq!(
                registry.child(root, a).unwrap().status,
                ChildStatus::Running
            );
            assert_eq!(
                registry.child(root, b).unwrap().status,
                ChildStatus::Running
            );
        });
    }

    #[gpui::test]
    fn test_concurrency_cap_counts_running_children_app_wide(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let first_orchestrator = ThreadId::new();
        let second_orchestrator = ThreadId::new();
        let first = register(&registry, first_orchestrator, directory.path(), "a", cx);
        let second = register(&registry, second_orchestrator, directory.path(), "b", cx);
        registry.update(cx, |registry, cx| {
            registry.set_status(first_orchestrator, first, ChildStatus::Running, cx);
            registry.set_status(second_orchestrator, second, ChildStatus::Running, cx);
            // The cap is app-wide, across orchestrators.
            let error = registry.reserve_child_slot(2).unwrap_err().to_string();
            assert!(error.contains("2 of 2"), "{error}");
            assert!(error.contains("max_concurrent_children"), "{error}");
            assert!(error.contains("await_children"), "{error}");

            // A finished child frees its slot.
            registry.record_turn_finished(
                first_orchestrator,
                first,
                ChildStatus::Completed,
                None,
                cx,
            );
            assert_eq!(registry.running_children(), 1);
            registry.reserve_child_slot(2).unwrap();
            // A start that has not registered yet still holds its slot.
            assert!(registry.reserve_child_slot(2).is_err());
            registry.release_child_slot();
            registry.reserve_child_slot(2).unwrap();
            registry.release_child_slot();
            registry.release_child_slot();
        });
    }

    fn write_log(
        directory: &Path,
        orchestrator: ThreadId,
        child: ThreadId,
        entries: &[(LogDirection, &str, ChildStatus, Option<ThreadId>)],
    ) {
        let log = ChildLog::new(directory, orchestrator, child);
        for (direction, text, status, sibling) in entries {
            match sibling {
                Some(sibling) => log
                    .append_sibling(*direction, text, *status, *sibling)
                    .unwrap(),
                None => log.append(*direction, text, *status).unwrap(),
            }
        }
    }

    fn restored(directory: &Path, child: ThreadId, title: &str) -> RestoredChild {
        RestoredChild {
            child_id: child,
            title: title.to_string().into(),
            agent_id: "zed-agent".to_string(),
            model: Some("ollama/qwen".to_string()),
            log_dir: directory.to_path_buf(),
            orchestrator_title: "Orchestrator".to_string(),
        }
    }

    #[gpui::test]
    async fn test_registry_is_rebuilt_from_saved_children_after_a_restart(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let done = ThreadId::new();
        let failed = ThreadId::new();
        let unanswered = ThreadId::new();
        let without_log = ThreadId::new();
        let sibling = ThreadId::new();
        write_log(
            directory.path(),
            orchestrator,
            done,
            &[
                (LogDirection::ToChild, "task", ChildStatus::Starting, None),
                (
                    LogDirection::FromChild,
                    "first",
                    ChildStatus::Completed,
                    None,
                ),
                (LogDirection::ToChild, "more", ChildStatus::Running, None),
                (
                    LogDirection::FromChild,
                    "all done",
                    ChildStatus::Completed,
                    None,
                ),
                (
                    LogDirection::ToSibling,
                    "q",
                    ChildStatus::Running,
                    Some(sibling),
                ),
                (
                    LogDirection::FromSibling,
                    "a",
                    ChildStatus::Running,
                    Some(sibling),
                ),
            ],
        );
        write_log(
            directory.path(),
            orchestrator,
            failed,
            &[
                (LogDirection::ToChild, "task", ChildStatus::Starting, None),
                (LogDirection::FromChild, "boom", ChildStatus::Failed, None),
            ],
        );
        write_log(
            directory.path(),
            orchestrator,
            unanswered,
            &[(LogDirection::ToChild, "task", ChildStatus::Starting, None)],
        );

        registry.update(cx, |registry, cx| {
            for (child, title) in [
                (done, "done"),
                (failed, "failed"),
                (unanswered, "unanswered"),
                (without_log, "without log"),
            ] {
                assert!(registry.restore_child(
                    orchestrator,
                    restored(directory.path(), child, title),
                    cx
                ));
            }
            // Already known: nothing changes.
            assert!(!registry.restore_child(
                orchestrator,
                restored(directory.path(), done, "done"),
                cx
            ));
            assert_eq!(registry.children(orchestrator).len(), 4);

            let record = registry.child(orchestrator, done).unwrap();
            assert_eq!(record.status, ChildStatus::Completed);
            assert_eq!(record.final_message.as_deref(), Some("all done"));
            assert_eq!(record.turns, 2);
            assert_eq!(record.model.as_deref(), Some("ollama/qwen"));
            assert_eq!(
                registry.child(orchestrator, failed).unwrap().status,
                ChildStatus::Failed
            );
            let record = registry.child(orchestrator, unanswered).unwrap();
            assert_eq!(record.status, ChildStatus::Failed);
            assert!(
                record
                    .final_message
                    .as_deref()
                    .unwrap()
                    .contains("restarted")
            );
            let record = registry.child(orchestrator, without_log).unwrap();
            assert_eq!(record.status, ChildStatus::Completed);
            assert_eq!(record.final_message, None);
            assert_eq!(registry.orchestrator_of(done), Some(orchestrator));
            // Nothing runs after a restart.
            assert_eq!(registry.running_children(), 0);
        });

        // The tools know the children: whoami, child_status, await_children.
        let whoami = cx.update(|cx| registry.read(cx).whoami(orchestrator, cx));
        assert_eq!(whoami.role, ThreadRole::Orchestrator);
        assert_eq!(whoami.children.len(), 4);
        let child_whoami = cx.update(|cx| registry.read(cx).whoami(done, cx));
        assert_eq!(child_whoami.role, ThreadRole::Child);
        assert_eq!(child_whoami.parent.unwrap().title, "Orchestrator");
        let statuses = status_of(&registry, orchestrator, cx).await;
        assert_eq!(statuses[0].sibling_messages, 2);
        assert_eq!(statuses[0].turns, 2);
        let awaited = cx
            .update(|cx| {
                OrchestrationRegistry::await_children(
                    &registry,
                    orchestrator,
                    AwaitChildrenInput {
                        child_ids: vec![done.to_key_string()],
                        timeout_secs: Some(5),
                    },
                    cx,
                )
            })
            .await
            .unwrap();
        assert!(!awaited.timed_out);
        assert_eq!(
            awaited.children[0].final_message.as_deref(),
            Some("all done")
        );
    }

    #[gpui::test]
    async fn test_restored_children_keep_their_sibling_message_count(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = ThreadId::new();
        let second = ThreadId::new();
        write_log(
            directory.path(),
            orchestrator,
            first,
            &[
                (
                    LogDirection::ToSibling,
                    "q",
                    ChildStatus::Running,
                    Some(second),
                ),
                (
                    LogDirection::FromSibling,
                    "a",
                    ChildStatus::Running,
                    Some(second),
                ),
            ],
        );
        registry.update(cx, |registry, cx| {
            registry.restore_child(orchestrator, restored(directory.path(), first, "a"), cx);
            registry.restore_child(orchestrator, restored(directory.path(), second, "b"), cx);
        });
        let error = cx
            .update(|cx| {
                OrchestrationRegistry::send_to_sibling_with_limit(
                    &registry,
                    first,
                    sibling_input(second, "again", false),
                    2,
                    cx,
                )
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("limit reached"), "{error}");
    }

    #[gpui::test]
    fn test_session_tokens_are_revoked_with_their_thread(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let orchestrator = ThreadId::new();
        let child = ThreadId::new();
        let directory = tempfile::tempdir().unwrap();
        registry.update(cx, |registry, cx| {
            registry.register_child(orchestrator, new_child_with(directory.path(), child), cx);
            let pending = registry.mint_pending_session_token();
            registry.bind_session_token(&pending, "session-1");
            let other = registry.token_for_session("session-2");
            let thread_token = registry.mint_token(child);
            assert!(registry.is_session_token_live(&pending));
            assert!(registry.is_session_token_live(&other));

            registry.revoke_session_tokens("session-1");
            assert!(!registry.is_session_token_live(&pending));
            assert!(registry.is_session_token_live(&other));
            // The same session loaded again gets a new token.
            assert_ne!(registry.token_for_session("session-1"), pending);

            // Removing the thread revokes its thread token, too, and the
            // parent forgets the child.
            registry.remove_orchestrator(child, cx);
            assert_eq!(registry.resolve_token(&thread_token), None);
            assert!(registry.children(orchestrator).is_empty());
        });
    }

    fn new_child_with(log_dir: &Path, child: ThreadId) -> NewChild {
        NewChild {
            child_id: child,
            ..new_child(log_dir, "child")
        }
    }

    #[gpui::test]
    async fn test_removing_a_thread_ends_the_waits_on_it(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let first = register(&registry, orchestrator, directory.path(), "a", cx);
        let second = register(&registry, orchestrator, directory.path(), "b", cx);
        let host = setup_host(&registry, &[orchestrator, first, second], cx);
        host.idle.set(true);

        let waiting = send_sibling(&registry, first, second, "q", true, cx);
        cx.run_until_parked();
        registry.update(cx, |registry, cx| registry.remove_orchestrator(second, cx));
        let error = waiting.await.unwrap_err();
        assert!(error.to_string().contains("closed"), "{error}");
        registry.read_with(cx, |registry, _| {
            assert!(registry.list_siblings(first).unwrap().siblings.is_empty());
        });
    }

    #[test]
    fn test_child_header_mentions_the_sibling_tools() {
        let header = child_header("Refactor billing", 0);
        assert!(header.contains("`list_siblings`"));
        assert!(header.contains("`send_to_sibling`"));
    }

    fn register_with_quota(
        registry: &Entity<OrchestrationRegistry>,
        orchestrator: ThreadId,
        log_dir: &Path,
        granted: u32,
        cx: &mut TestAppContext,
    ) -> ThreadId {
        let child = new_child(log_dir, "limited");
        let child_id = child.child_id;
        registry.update(cx, |registry, cx| {
            registry.set_subagent_quota(
                child_id,
                granted,
                crate::child_quota_dir(log_dir, orchestrator, child_id),
            );
            registry.register_child(orchestrator, child, cx);
        });
        child_id
    }

    #[gpui::test]
    fn test_spawns_are_refused_beyond_the_quota_and_logged(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register_with_quota(&registry, orchestrator, directory.path(), 1, cx);

        registry
            .update(cx, |registry, _| {
                registry.consume_subagent_quota(child, "spawn_agent")
            })
            .unwrap();
        let error = registry
            .update(cx, |registry, _| {
                registry.consume_subagent_quota(child, "create_thread")
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("Subagent quota exhausted"), "{error}");

        let quota_lines = log_entries(directory.path(), orchestrator, child)
            .into_iter()
            .filter(|(direction, _)| *direction == LogDirection::Quota)
            .map(|(_, text)| text)
            .collect::<Vec<_>>();
        assert_eq!(
            quota_lines,
            [
                "subagent_quota granted: 1",
                "subagent_quota used: 1 of 1 (`spawn_agent`)",
                "subagent_quota refused: `create_thread`",
            ]
        );
    }

    #[gpui::test]
    fn test_the_quota_defaults_to_zero_and_is_per_child(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let none = register(&registry, orchestrator, directory.path(), "none", cx);
        let some = register_with_quota(&registry, orchestrator, directory.path(), 2, cx);

        registry.update(cx, |registry, _| {
            assert!(
                registry
                    .consume_subagent_quota(none, "start_child")
                    .is_err()
            );
            registry
                .consume_subagent_quota(some, "start_child")
                .unwrap();
            registry
                .consume_subagent_quota(some, "start_child")
                .unwrap();
            assert!(
                registry
                    .consume_subagent_quota(some, "start_child")
                    .is_err()
            );
            // A thread that is not an orchestrated child has no quota.
            registry
                .consume_subagent_quota(ThreadId::new(), "start_child")
                .unwrap();
        });
    }

    #[gpui::test]
    async fn test_child_status_reports_the_quota_used_and_granted(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register_with_quota(&registry, orchestrator, directory.path(), 3, cx);
        registry
            .update(cx, |registry, _| {
                registry.consume_subagent_quota(child, "spawn_agent")
            })
            .unwrap();

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
        assert_eq!(
            output.children[0].subagent_quota,
            SubagentQuotaStatus {
                granted: 3,
                used: 1
            }
        );
        let serialized = serde_json::to_value(&output).unwrap();
        assert_eq!(
            serialized["children"][0]["subagent_quota"],
            serde_json::json!({"granted": 3, "used": 1})
        );
    }

    #[gpui::test]
    fn test_announced_subagents_over_quota_are_cancelled_and_logged(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register_with_quota(&registry, orchestrator, directory.path(), 1, cx);

        registry.update(cx, |registry, _| {
            assert!(!registry.check_announced_subagent(child, "sub-1", true, true));
            assert!(registry.check_announced_subagent(child, "sub-2", true, true));
            // Later updates of the same subagent do not cancel it again.
            assert!(!registry.check_announced_subagent(child, "sub-2", true, true));
            // Without `capabilities.cancel` nothing is cancelled.
            assert!(!registry.check_announced_subagent(child, "sub-3", false, true));
        });
        let events = log_entries(directory.path(), orchestrator, child)
            .into_iter()
            .filter(|(direction, _)| *direction == LogDirection::Quota)
            .map(|(_, text)| text)
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 3, "{events:?}");
        assert!(events[1].contains("cancelled subagent sub-2"), "{events:?}");
        assert!(
            events[2].contains("does not allow cancelling it"),
            "{events:?}"
        );
        registry.read_with(cx, |registry, _| {
            assert_eq!(registry.subagent_quota(child).unwrap().used(), 3);
        });
    }

    #[gpui::test]
    async fn test_the_quota_survives_a_restart_through_the_log(cx: &mut TestAppContext) {
        let registry = init_registry(cx);
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = register_with_quota(&registry, orchestrator, directory.path(), 2, cx);
        registry
            .update(cx, |registry, _| {
                registry.consume_subagent_quota(child, "spawn_agent")
            })
            .unwrap();
        registry.update(cx, |registry, cx| {
            registry.remove_orchestrator(orchestrator, cx)
        });
        registry.read_with(cx, |registry, _| {
            assert!(registry.subagent_quota(child).is_none());
        });

        registry.update(cx, |registry, cx| {
            registry.restore_child(
                orchestrator,
                restored(directory.path(), child, "limited"),
                cx,
            );
        });
        registry.update(cx, |registry, _| {
            let quota = registry.subagent_quota(child).unwrap();
            assert_eq!((quota.granted(), quota.used()), (2, 1));
            registry
                .consume_subagent_quota(child, "spawn_agent")
                .unwrap();
            assert!(
                registry
                    .consume_subagent_quota(child, "spawn_agent")
                    .is_err()
            );
        });
    }

    #[test]
    fn test_child_header_states_the_quota() {
        assert!(child_header("Billing", 0).contains("granted you no subagents"));
        assert!(child_header("Billing", 1).contains("granted you 1 subagent:"));
        let header = child_header("Billing", 3);
        assert!(header.contains("granted you 3 subagents"));
        assert!(header.contains("at most 3"));
    }

    #[test]
    fn test_split_delivered_text_separates_what_the_model_needs_from_the_message() {
        let first = format!("{}fix the build", child_header("Refactor billing", 0));
        let parts = crate::split_delivered_text(&first);
        assert_eq!(parts.body, "fix the build");
        assert!(
            parts
                .context
                .unwrap()
                .starts_with(crate::CHILD_HEADER_MARKER)
        );

        let sibling = sibling_message_text("api", "abc", "which file?", false);
        let parts = crate::split_delivered_text(&sibling);
        assert_eq!(parts.body, "which file?");
        assert!(parts.context.unwrap().contains("`send_to_sibling`"));

        let parts = crate::split_delivered_text("[from child worker] blocked");
        assert_eq!(parts.body, "blocked");
        assert_eq!(parts.context, None);

        let parts = crate::split_delivered_text("plain follow-up");
        assert_eq!(parts.body, "plain follow-up");
        assert_eq!(parts.context, None);
    }
}
