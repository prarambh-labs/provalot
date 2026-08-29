use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Line {
    Run {
        ts: u64,
        agent_id: Option<String>,
        tool: String,
        command: String,
        runner: String,
        outcome: String,
        stdout_hash: String,
        stderr_hash: String,
        is_error: bool,
        interrupted: bool,
    },
    EditPending {
        ts: u64,
        agent_id: Option<String>,
        tool_use_id: Option<String>,
        path: String,
        hash_before: Option<String>,
    },
    Edit {
        ts: u64,
        agent_id: Option<String>,
        tool: String,
        path: String,
        hash_before: Option<String>,
        hash_after: Option<String>,
        changed: bool,
    },
    Claim {
        ts: u64,
        agent_id: Option<String>,
        event: String,
        class: String,
        text: String,
        path: Option<String>,
    },
    Decision {
        ts: u64,
        agent_id: Option<String>,
        event: String,
        decision: String,
        rule: String,
        reason: String,
        consecutive: u32,
    },
    Snapshot {
        ts: u64,
        runs: u32,
        passes: u32,
        fails: u32,
        edits_changed: u32,
        claims: u32,
    },
    Override {
        ts: u64,
    },
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn sanitize(session_id: &str) -> String {
    let s: String = session_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() {
        "unknown".to_string()
    } else {
        s
    }
}

pub fn sessions_dir(root: &Path) -> PathBuf {
    root.join(".provalot").join("sessions")
}

pub fn path(root: &Path, session_id: &str) -> PathBuf {
    sessions_dir(root).join(format!("{}.jsonl", sanitize(session_id)))
}

pub fn append(root: &Path, session_id: &str, line: &Line) -> Result<(), String> {
    let p = path(root, session_id);
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
        .map_err(|e| e.to_string())?;
    let json = serde_json::to_string(line).map_err(|e| e.to_string())?;
    writeln!(f, "{json}").map_err(|e| e.to_string())
}

pub fn read(root: &Path, session_id: &str) -> Vec<Line> {
    let p = path(root, session_id);
    let Ok(f) = fs::File::open(&p) else {
        return Vec::new();
    };
    BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect()
}

/// Session id (file stem) of the most recently modified ledger, if any.
pub fn latest_session(root: &Path) -> Option<String> {
    let entries = fs::read_dir(sessions_dir(root)).ok()?;
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(m) = e.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        let stem = p.file_stem()?.to_string_lossy().to_string();
        if best.as_ref().map(|(t, _)| m > *t).unwrap_or(true) {
            best = Some((m, stem));
        }
    }
    best.map(|(_, s)| s)
}

pub fn all_sessions(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(sessions_dir(root)) else {
        return Vec::new();
    };
    let mut v: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension().and_then(|x| x.to_str()) == Some("jsonl"))
                .then(|| p.file_stem().map(|s| s.to_string_lossy().to_string()))
                .flatten()
        })
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_then_read_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let a = Line::Run {
            ts: 1,
            agent_id: None,
            tool: "Bash".into(),
            command: "pytest".into(),
            runner: "pytest".into(),
            outcome: "pass".into(),
            stdout_hash: "h1".into(),
            stderr_hash: "h2".into(),
            is_error: false,
            interrupted: false,
        };
        let b = Line::Override { ts: 2 };
        append(dir.path(), "s1", &a).unwrap();
        append(dir.path(), "s1", &b).unwrap();
        assert_eq!(read(dir.path(), "s1"), vec![a, b]);
        assert_eq!(read(dir.path(), "missing"), Vec::<Line>::new());
    }

    #[test]
    fn sanitizes_session_ids() {
        assert_eq!(sanitize("abc-123_x.y"), "abc-123_x.y");
        assert_eq!(sanitize("../evil id"), ".._evil_id");
        assert_eq!(sanitize(""), "unknown");
    }

    #[test]
    fn latest_session_is_most_recently_modified() {
        let dir = tempfile::tempdir().unwrap();
        append(dir.path(), "old", &Line::Override { ts: 1 }).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        append(dir.path(), "new", &Line::Override { ts: 2 }).unwrap();
        assert_eq!(latest_session(dir.path()).as_deref(), Some("new"));
    }
}
