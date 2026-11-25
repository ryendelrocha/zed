//! The subagent quota of R6: how many subagents or children a child may start.
//!
//! Enforcement is layered. This module holds the pieces that do not depend on
//! the UI: the per-child counter ([`SubagentQuota`], layers L1 and L4), the
//! Claude Code session options (layers L2 and L3), and the hand-over of a
//! child's grant to the ACP session that Zed is about to create.

use std::{
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use anyhow::{Context as _, Result};
use gpui::{App, Global, Task};
use serde_json::{Map, Value, json};
use util::ResultExt as _;

use crate::{
    OrchestrationHost, OrchestrationRegistry, StartChildInput, StartChildOutput, ThreadId,
};

/// Names of Claude Code's tool that starts a subagent. Claude Code renamed
/// `Task` to `Agent`; the ACP adapter treats both as the subagent tool
/// (`isNativeSubagentControlTool` in `claude-agent-acp`, `src/native-subagents.ts`).
pub const CLAUDE_CODE_SUBAGENT_TOOLS: [&str; 2] = ["Agent", "Task"];

const TICKET_PREFIX: &str = "ticket-";

/// Directory of the quota tickets of a child, next to its R4 log:
/// `<log_dir>/<orchestrator-id>/<child-id>.quota`.
pub fn child_quota_dir(log_dir: &Path, orchestrator: ThreadId, child: ThreadId) -> PathBuf {
    log_dir
        .join(orchestrator.to_key_string())
        .join(format!("{}.quota", child.to_key_string()))
}

/// How many subagents a child was granted, and how many it has used.
///
/// Every way a child can start a subagent ends up here:
/// - L1: Zed's own tools call [`Self::consume`].
/// - L3: the Claude Code hook takes one ticket file per `Agent` call, so the
///   tickets left say how many calls the hook still allows.
/// - L4: the ACP `subagent_update` announcements are recorded with
///   [`Self::note_announced`].
#[derive(Clone, Debug)]
pub struct SubagentQuota {
    granted: u32,
    spawned: u32,
    tickets_dir: Option<PathBuf>,
    announced: Vec<String>,
    cancelled: Vec<String>,
    reported_uncancellable: Vec<String>,
}

/// What to do with a subagent that an agent announced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnouncedSubagent {
    /// Within the quota, or already handled.
    Allow,
    /// Over the quota and the agent lets Zed cancel it: cancel it now.
    Cancel,
    /// Over the quota, but the agent did not advertise `capabilities.cancel`.
    /// Nothing can be done beyond logging it.
    OverQuotaNotCancellable,
}

impl SubagentQuota {
    pub fn new(granted: u32, tickets_dir: Option<PathBuf>) -> Self {
        Self {
            granted,
            spawned: 0,
            tickets_dir,
            announced: Vec::new(),
            cancelled: Vec::new(),
            reported_uncancellable: Vec::new(),
        }
    }

    /// A quota rebuilt after a restart: `used` spawns are already counted.
    pub fn restored(granted: u32, used: u32, tickets_dir: Option<PathBuf>) -> Self {
        Self {
            spawned: used,
            ..Self::new(granted, tickets_dir)
        }
    }

    pub fn granted(&self) -> u32 {
        self.granted
    }

    pub fn tickets_dir(&self) -> Option<&Path> {
        self.tickets_dir.as_deref()
    }

    /// The subagents started so far, by any layer. The largest of the counts
    /// the layers keep, because one subagent can be seen by more than one
    /// layer (a Claude Code `Agent` call passes the hook and is then
    /// announced).
    pub fn used(&self) -> u32 {
        self.spawned
            .max(self.tickets_taken())
            .max(self.announced.len() as u32)
    }

    fn tickets_taken(&self) -> u32 {
        let Some(directory) = &self.tickets_dir else {
            return 0;
        };
        if !directory.is_dir() {
            return 0;
        }
        self.granted.saturating_sub(count_tickets(directory))
    }

    /// L1. Counts one subagent started through a Zed tool, or refuses it.
    pub fn consume(&mut self, tool: &str) -> Result<()> {
        let used = self.used();
        if used >= self.granted {
            return Err(anyhow::anyhow!(
                "Subagent quota exhausted: your orchestrator granted this thread {granted} \
                 subagent(s) and {used} are used, so `{tool}` is refused. Do the work yourself, \
                 or ask the orchestrator for more with `send_to_parent`.",
                granted = self.granted,
            ));
        }
        self.spawned += 1;
        self.take_ticket();
        Ok(())
    }

    /// Gives back what [`Self::consume`] counted, when the start failed.
    pub fn refund(&mut self) {
        if self.spawned == 0 {
            return;
        }
        self.spawned -= 1;
        self.give_back_ticket();
    }

    fn take_ticket(&self) {
        let Some(directory) = self
            .tickets_dir
            .as_ref()
            .filter(|directory| directory.is_dir())
        else {
            return;
        };
        let Some(ticket) = ticket_paths(directory).into_iter().next() else {
            return;
        };
        fs::remove_file(&ticket)
            .with_context(|| format!("taking quota ticket {}", ticket.display()))
            .log_err();
    }

    fn give_back_ticket(&self) {
        let Some(directory) = self
            .tickets_dir
            .as_ref()
            .filter(|directory| directory.is_dir())
        else {
            return;
        };
        let ticket = directory.join(format!("{TICKET_PREFIX}refund-{}", uuid::Uuid::new_v4()));
        fs::write(&ticket, b"")
            .with_context(|| format!("returning quota ticket {}", ticket.display()))
            .log_err();
    }

    /// L4. Records a subagent the agent announced for this child and says
    /// what to do about it. The subagents announced after the quota is used up
    /// are the extra ones. Call it on every update of the subagent, so a
    /// cancel grant, or the start of its work, that comes later still takes
    /// effect: a subagent that is not `live` (running) has nothing to cancel
    /// yet and is looked at again.
    pub fn note_announced(
        &mut self,
        subagent: &str,
        can_cancel: bool,
        live: bool,
    ) -> AnnouncedSubagent {
        let index = match self.announced.iter().position(|known| known == subagent) {
            Some(index) => index,
            None => {
                self.announced.push(subagent.to_string());
                self.announced.len() - 1
            }
        };
        if (index as u32) < self.granted || self.cancelled.iter().any(|known| known == subagent) {
            return AnnouncedSubagent::Allow;
        }
        if can_cancel {
            if !live {
                return AnnouncedSubagent::Allow;
            }
            self.cancelled.push(subagent.to_string());
            return AnnouncedSubagent::Cancel;
        }
        if self
            .reported_uncancellable
            .iter()
            .any(|known| known == subagent)
        {
            return AnnouncedSubagent::Allow;
        }
        self.reported_uncancellable.push(subagent.to_string());
        AnnouncedSubagent::OverQuotaNotCancellable
    }
}

fn ticket_paths(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut tickets = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(TICKET_PREFIX)
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    tickets.sort();
    tickets
}

/// How many tickets are left in `directory`.
pub fn count_tickets(directory: &Path) -> u32 {
    ticket_paths(directory).len() as u32
}

/// The command of the Claude Code `PreToolUse` hook (L3). It takes one ticket
/// file with `rm`, which succeeds for exactly one of any number of concurrent
/// hooks, and exits with code 2 (a blocking error, whose stderr goes to the
/// model) when none is left.
pub fn quota_hook_command(tickets_dir: &Path) -> String {
    let directory = tickets_dir.display().to_string().replace('\'', r"'\''");
    format!(
        "cat >/dev/null; for ticket in '{directory}'/{TICKET_PREFIX}*; do \
         rm \"$ticket\" 2>/dev/null && exit 0; done; \
         echo 'Subagent quota exhausted: your orchestrator did not grant you more subagents. \
         Do the work yourself, or ask the orchestrator for more with send_to_parent.' >&2; exit 2"
    )
}

/// What a child's ACP session needs to enforce its quota.
#[derive(Clone, Debug)]
pub struct SessionQuotaGrant {
    pub granted: u32,
    pub tickets_dir: PathBuf,
}

/// The `_meta` of Claude Code's `session/new` for a child with `grant` (L2 and
/// L3). The adapter reads `_meta.claudeCode.options` and merges it into the
/// Claude Code SDK options (`claude-agent-acp`, `src/v1/acp-agent.ts`):
///
/// - quota 0: `disallowedTools` removes the subagent tool;
/// - quota N: `settings` carries a `PreToolUse` command hook on the subagent
///   tool, and `N` ticket files are written for it to take. The adapter's own
///   `hooks` option accepts callbacks only, so a command hook has to travel as
///   settings.
pub fn claude_code_quota_meta(grant: &SessionQuotaGrant) -> Result<Map<String, Value>> {
    let options = if grant.granted == 0 {
        json!({ "disallowedTools": CLAUDE_CODE_SUBAGENT_TOOLS })
    } else {
        write_tickets(&grant.tickets_dir, grant.granted)?;
        json!({
            "settings": {
                "hooks": {
                    "PreToolUse": [{
                        "matcher": CLAUDE_CODE_SUBAGENT_TOOLS.join("|"),
                        "hooks": [{
                            "type": "command",
                            "command": quota_hook_command(&grant.tickets_dir),
                        }],
                    }],
                },
            },
        })
    };
    let mut meta = Map::new();
    meta.insert("claudeCode".to_string(), json!({ "options": options }));
    Ok(meta)
}

fn write_tickets(directory: &Path, count: u32) -> Result<()> {
    fs::create_dir_all(directory)
        .with_context(|| format!("creating quota directory {}", directory.display()))?;
    for ticket in ticket_paths(directory) {
        fs::remove_file(&ticket)
            .with_context(|| format!("clearing quota ticket {}", ticket.display()))?;
    }
    for number in 1..=count {
        let ticket = directory.join(format!("{TICKET_PREFIX}{number}"));
        fs::write(&ticket, b"")
            .with_context(|| format!("writing quota ticket {}", ticket.display()))?;
    }
    Ok(())
}

/// The grant of the child whose ACP session is about to be created. It is set
/// right before `new_session` and taken at the start of it, in the same
/// synchronous call, so it cannot reach another session.
#[derive(Default)]
struct StagedSessionQuota(Option<SessionQuotaGrant>);

impl Global for StagedSessionQuota {}

/// Hands the grant of `thread`, if it is an orchestrated child, to the next
/// `new_session` call. Call [`clear_staged_session_quota`] after it.
pub fn stage_session_quota(thread: ThreadId, cx: &mut App) {
    let grant = OrchestrationRegistry::try_global(cx)
        .and_then(|registry| registry.read(cx).session_quota_grant(thread));
    cx.set_global(StagedSessionQuota(grant));
}

/// Takes the staged grant. Connections that apply it call this at the start of
/// `new_session`.
pub fn take_staged_session_quota(cx: &mut App) -> Option<SessionQuotaGrant> {
    cx.try_global::<StagedSessionQuota>()?;
    cx.global_mut::<StagedSessionQuota>().0.take()
}

pub fn clear_staged_session_quota(cx: &mut App) {
    if cx.has_global::<StagedSessionQuota>() {
        cx.global_mut::<StagedSessionQuota>().0 = None;
    }
}

/// Starts a child on behalf of `caller` under `caller`'s own subagent quota
/// (L1). The native `start_child` tool and the MCP one both go through it.
pub fn start_child_within_quota(
    host: Rc<dyn OrchestrationHost>,
    caller: ThreadId,
    input: StartChildInput,
    cx: &mut App,
) -> Task<Result<StartChildOutput>> {
    let registry = OrchestrationRegistry::global_or_init(cx);
    if let Err(error) = registry.update(cx, |registry, _| {
        registry.consume_subagent_quota(caller, "start_child")
    }) {
        return Task::ready(Err(error));
    }
    let started = host.start_child(caller, input, cx);
    let registry = registry.downgrade();
    cx.spawn(async move |cx| match started.await {
        Ok(output) => Ok(output),
        Err(error) => {
            registry
                .update(cx, |registry, _| registry.refund_subagent_quota(caller))
                .log_err();
            Err(error)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consume_refuses_beyond_the_quota_with_a_clear_error() {
        let mut quota = SubagentQuota::new(1, None);
        quota.consume("spawn_agent").unwrap();
        let error = quota.consume("create_thread").unwrap_err().to_string();
        assert!(error.contains("Subagent quota exhausted"), "{error}");
        assert!(
            error.contains("granted this thread 1 subagent(s)"),
            "{error}"
        );
        assert!(error.contains("`create_thread` is refused"), "{error}");
        assert_eq!(quota.used(), 1);
    }

    #[test]
    fn test_quota_zero_refuses_the_first_spawn() {
        let mut quota = SubagentQuota::new(0, None);
        assert!(quota.consume("spawn_agent").is_err());
        assert_eq!(quota.used(), 0);
    }

    #[test]
    fn test_refund_gives_the_spawn_back() {
        let mut quota = SubagentQuota::new(1, None);
        quota.consume("start_child").unwrap();
        quota.refund();
        quota.consume("start_child").unwrap();
        assert!(quota.consume("start_child").is_err());
    }

    #[test]
    fn test_hook_tickets_and_zed_tools_share_one_budget() {
        let directory = tempfile::tempdir().unwrap();
        let tickets = directory.path().join("child.quota");
        let grant = SessionQuotaGrant {
            granted: 2,
            tickets_dir: tickets.clone(),
        };
        claude_code_quota_meta(&grant).unwrap();
        let mut quota = SubagentQuota::new(2, Some(tickets.clone()));
        assert_eq!(quota.used(), 0);

        // The hook lets one `Agent` call through.
        fs::remove_file(ticket_paths(&tickets)[0].clone()).unwrap();
        assert_eq!(quota.used(), 1);

        // A Zed tool takes the second one.
        quota.consume("start_child").unwrap();
        assert_eq!(quota.used(), 2);
        assert_eq!(count_tickets(&tickets), 0);
        assert!(quota.consume("spawn_agent").is_err());
    }

    #[test]
    fn test_quota_zero_meta_disallows_the_claude_code_subagent_tool() {
        let meta = claude_code_quota_meta(&SessionQuotaGrant {
            granted: 0,
            tickets_dir: PathBuf::from("/unused"),
        })
        .unwrap();
        let options = &meta["claudeCode"]["options"];
        assert_eq!(options["disallowedTools"], json!(["Agent", "Task"]));
        assert!(options.get("settings").is_none());
    }

    #[test]
    fn test_quota_n_meta_carries_a_pre_tool_use_command_hook_and_tickets() {
        let directory = tempfile::tempdir().unwrap();
        let tickets = directory.path().join("child.quota");
        let meta = claude_code_quota_meta(&SessionQuotaGrant {
            granted: 2,
            tickets_dir: tickets.clone(),
        })
        .unwrap();
        let options = &meta["claudeCode"]["options"];
        assert!(options.get("disallowedTools").is_none());
        let hook = &options["settings"]["hooks"]["PreToolUse"][0];
        assert_eq!(hook["matcher"], "Agent|Task");
        assert_eq!(hook["hooks"][0]["type"], "command");
        let command = hook["hooks"][0]["command"].as_str().unwrap();
        assert!(command.contains(&tickets.display().to_string()));
        assert!(command.ends_with("exit 2"));
        assert_eq!(count_tickets(&tickets), 2);
    }

    #[cfg(unix)]
    #[test]
    fn test_hook_command_allows_n_calls_then_blocks_with_exit_code_2() {
        let directory = tempfile::tempdir().unwrap();
        let tickets = directory.path().join("it's a child.quota");
        let meta = claude_code_quota_meta(&SessionQuotaGrant {
            granted: 2,
            tickets_dir: tickets.clone(),
        })
        .unwrap();
        let command = meta["claudeCode"]["options"]["settings"]["hooks"]["PreToolUse"][0]["hooks"]
            [0]["command"]
            .as_str()
            .unwrap()
            .to_string();
        let run = || {
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap()
        };
        assert_eq!(run().status.code(), Some(0));
        assert_eq!(run().status.code(), Some(0));
        let denied = run();
        assert_eq!(denied.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&denied.stderr).contains("Subagent quota exhausted"),
            "{}",
            String::from_utf8_lossy(&denied.stderr)
        );
        assert_eq!(count_tickets(&tickets), 0);
    }

    #[test]
    fn test_announced_subagents_beyond_the_quota_are_cancelled_once() {
        let mut quota = SubagentQuota::new(1, None);
        assert_eq!(
            quota.note_announced("a", true, true),
            AnnouncedSubagent::Allow
        );
        assert_eq!(
            quota.note_announced("b", true, true),
            AnnouncedSubagent::Cancel
        );
        assert_eq!(
            quota.note_announced("b", true, true),
            AnnouncedSubagent::Allow
        );
        assert_eq!(
            quota.note_announced("a", true, true),
            AnnouncedSubagent::Allow
        );
        assert_eq!(quota.used(), 2);
    }

    #[test]
    fn test_over_quota_without_the_cancel_capability_is_reported_but_not_cancelled() {
        let mut quota = SubagentQuota::new(0, None);
        assert_eq!(
            quota.note_announced("a", false, true),
            AnnouncedSubagent::OverQuotaNotCancellable
        );
        assert_eq!(
            quota.note_announced("a", false, true),
            AnnouncedSubagent::Allow
        );
        // The grant arrives later, but the subagent is not running yet.
        assert_eq!(
            quota.note_announced("a", true, false),
            AnnouncedSubagent::Allow
        );
        // Once it runs, it can be cancelled.
        assert_eq!(
            quota.note_announced("a", true, true),
            AnnouncedSubagent::Cancel
        );
    }
}
