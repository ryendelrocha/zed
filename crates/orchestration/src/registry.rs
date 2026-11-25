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
    AwaitChildrenInput, AwaitChildrenOutput, ChildLog, ChildResult, ChildStatus, ChildStatusInfo,
    ChildStatusInput, ChildStatusOutput, DEFAULT_AWAIT_TIMEOUT_SECS, LogDirection,
    MAX_AWAIT_TIMEOUT_SECS, OrchestrationHost, ThreadId, log_dir_from_setting,
};

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
    log: ChildLog,
    status_sender: watch::Sender<ChildStatus>,
}

impl ChildRecord {
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
                log,
                status_sender,
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

    /// Metadata only: status, counters and times, never content.
    pub fn child_status(
        &self,
        orchestrator: ThreadId,
        input: &ChildStatusInput,
    ) -> Result<ChildStatusOutput> {
        let children = self
            .resolve_child_ids(orchestrator, &input.child_ids)?
            .into_iter()
            .filter_map(|child| self.child(orchestrator, child))
            .map(|record| ChildStatusInfo {
                child_id: record.child_id.to_key_string(),
                title: record.title.to_string(),
                agent: record.agent_id.clone(),
                model: record.model.clone(),
                status: record.status,
                turns: record.turns,
                last_activity: record.last_activity.to_rfc3339(),
            })
            .collect();
        Ok(ChildStatusOutput { children })
    }

    /// Waits until each requested child is in a terminal state, or until the
    /// timeout, and returns each child's status and final message only.
    ///
    /// Children that already finished return at once. Children that are
    /// still running when the timeout expires come back with their current
    /// status and no message, and `timed_out` set.
    pub fn await_children(
        registry: &Entity<Self>,
        orchestrator: ThreadId,
        input: AwaitChildrenInput,
        cx: &mut App,
    ) -> Task<Result<AwaitChildrenOutput>> {
        let (child_ids, receivers) = {
            let this = registry.read(cx);
            let child_ids = match this.resolve_child_ids(orchestrator, &input.child_ids) {
                Ok(child_ids) => child_ids,
                Err(error) => return Task::ready(Err(error)),
            };
            let receivers = child_ids
                .iter()
                .filter_map(|child| this.child(orchestrator, *child))
                .map(ChildRecord::subscribe_status)
                .collect::<Vec<_>>();
            (child_ids, receivers)
        };
        let timeout = Duration::from_secs(
            input
                .timeout_secs
                .unwrap_or(DEFAULT_AWAIT_TIMEOUT_SECS)
                .min(MAX_AWAIT_TIMEOUT_SECS),
        );
        let timer = cx.background_executor().timer(timeout);
        let registry = registry.downgrade();

        cx.spawn(async move |cx| {
            let all_finished =
                future::join_all(receivers.into_iter().map(wait_until_terminal)).map(|_| ());
            let timed_out = match future::select(Box::pin(all_finished), Box::pin(timer)).await {
                future::Either::Left(_) => false,
                future::Either::Right(_) => true,
            };
            registry.read_with(cx, |registry, _| {
                let children = child_ids
                    .iter()
                    .filter_map(|child| registry.child(orchestrator, *child))
                    .map(|record| ChildResult {
                        child_id: record.child_id.to_key_string(),
                        title: record.title.to_string(),
                        status: record.status,
                        final_message: record
                            .status
                            .is_terminal()
                            .then(|| record.final_message.clone())
                            .flatten(),
                    })
                    .collect::<Vec<_>>();
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
    use gpui::TestAppContext;

    fn new_child(log_dir: &Path, title: &str) -> NewChild {
        NewChild {
            child_id: ThreadId::new(),
            title: title.to_string().into(),
            agent_id: "zed-agent".to_string(),
            model: None,
            log_dir: log_dir.to_path_buf(),
            prompt: format!("prompt for {title}"),
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
    fn test_child_status_returns_metadata_only(cx: &mut TestAppContext) {
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

        let output = registry.read_with(cx, |registry, _| {
            registry
                .child_status(orchestrator, &ChildStatusInput::default())
                .unwrap()
        });
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
}
