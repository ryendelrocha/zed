use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{ChildStatus, ThreadId};

/// Who sent a logged message.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogDirection {
    /// From the orchestrator to the child: the first prompt and follow-ups.
    ToChild,
    /// From the child to the orchestrator: the final message of a turn, or
    /// the reason the child failed.
    FromChild,
    /// From the child to the orchestrator, through `send_to_parent`.
    ToParent,
    /// The summary of what a human did in the child. The human's own
    /// messages are never logged.
    HumanIntervention,
    /// From this child to a sibling, through `send_to_sibling`. The entry's
    /// `sibling` is the receiver.
    ToSibling,
    /// From a sibling to this child. The entry's `sibling` is the sender.
    FromSibling,
}

/// One line of a child's log file.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ChildLogEntry {
    pub ts: DateTime<Utc>,
    pub direction: LogDirection,
    pub text: String,
    pub status: ChildStatus,
    /// The other child of a sibling exchange (`to_sibling` or
    /// `from_sibling`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sibling: Option<String>,
}

/// Directory used when `agent.orchestration.log_dir` is not set.
pub fn default_log_dir() -> PathBuf {
    std::env::temp_dir().join("zed-orchestration")
}

/// Resolves the `agent.orchestration.log_dir` setting. Empty values count as
/// unset.
pub fn log_dir_from_setting(setting: Option<&str>) -> PathBuf {
    match setting.map(str::trim) {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => default_log_dir(),
    }
}

/// JSONL log of the exchanges between an orchestrator and one child, at
/// `<log_dir>/<orchestrator-id>/<child-id>.jsonl`.
///
/// Only the messages that cross between the two are written: never the
/// child's tool calls or intermediate output. The writes are a few hundred
/// bytes at turn boundaries, so they are synchronous.
#[derive(Clone, Debug)]
pub struct ChildLog {
    path: PathBuf,
}

impl ChildLog {
    pub fn new(log_dir: &Path, orchestrator: ThreadId, child: ThreadId) -> Self {
        Self {
            path: log_dir
                .join(orchestrator.to_key_string())
                .join(format!("{}.jsonl", child.to_key_string())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, direction: LogDirection, text: &str, status: ChildStatus) -> Result<()> {
        self.append_entry(direction, text, status, None)
    }

    /// Appends a sibling exchange. `sibling` is the id of the other child.
    pub fn append_sibling(
        &self,
        direction: LogDirection,
        text: &str,
        status: ChildStatus,
        sibling: ThreadId,
    ) -> Result<()> {
        self.append_entry(direction, text, status, Some(sibling.to_key_string()))
    }

    fn append_entry(
        &self,
        direction: LogDirection,
        text: &str,
        status: ChildStatus,
        sibling: Option<String>,
    ) -> Result<()> {
        let entry = ChildLogEntry {
            ts: Utc::now(),
            direction,
            text: text.to_string(),
            status,
            sibling,
        };
        let parent = self
            .path
            .parent()
            .context("child log path has no parent directory")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("creating log directory {}", parent.display()))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("opening log file {}", self.path.display()))?;
        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .with_context(|| format!("writing log file {}", self.path.display()))
    }

    /// Reads every entry back. A missing file is an empty log.
    pub fn read(&self) -> Result<Vec<ChildLogEntry>> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("reading log file {}", self.path.display()));
            }
        };
        contents
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).context("parsing log line"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_dir_setting_override_and_default() {
        assert_eq!(
            log_dir_from_setting(Some("/tmp/custom-logs")),
            PathBuf::from("/tmp/custom-logs")
        );
        assert_eq!(log_dir_from_setting(None), default_log_dir());
        assert_eq!(log_dir_from_setting(Some("  ")), default_log_dir());
        assert!(default_log_dir().ends_with("zed-orchestration"));
    }

    #[test]
    fn test_append_writes_one_json_line_per_exchange() {
        let directory = tempfile::tempdir().unwrap();
        let orchestrator = ThreadId::new();
        let child = ThreadId::new();
        let log = ChildLog::new(directory.path(), orchestrator, child);
        assert_eq!(
            log.path(),
            directory
                .path()
                .join(orchestrator.to_key_string())
                .join(format!("{}.jsonl", child.to_key_string()))
        );
        assert!(log.read().unwrap().is_empty());

        log.append(LogDirection::ToChild, "do it", ChildStatus::Starting)
            .unwrap();
        log.append(LogDirection::FromChild, "done", ChildStatus::Completed)
            .unwrap();

        let raw = fs::read_to_string(log.path()).unwrap();
        assert_eq!(raw.lines().count(), 2);
        let entries = log.read().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].direction, LogDirection::ToChild);
        assert_eq!(entries[0].text, "do it");
        assert_eq!(entries[1].direction, LogDirection::FromChild);
        assert_eq!(entries[1].status, ChildStatus::Completed);
        assert_eq!(entries[1].sibling, None);
        assert!(!raw.contains("sibling"));
    }

    #[test]
    fn test_sibling_entries_carry_the_other_child() {
        let directory = tempfile::tempdir().unwrap();
        let log = ChildLog::new(directory.path(), ThreadId::new(), ThreadId::new());
        let other = ThreadId::new();
        log.append_sibling(
            LogDirection::ToSibling,
            "which file?",
            ChildStatus::Running,
            other,
        )
        .unwrap();
        let entries = log.read().unwrap();
        assert_eq!(entries[0].direction, LogDirection::ToSibling);
        assert_eq!(entries[0].sibling, Some(other.to_key_string()));
        assert!(
            fs::read_to_string(log.path())
                .unwrap()
                .contains("\"direction\":\"to_sibling\"")
        );
    }
}
