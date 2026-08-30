use std::path::{Path, PathBuf};

use crate::adapters;
use crate::claims;
use crate::decide::{self, Verdict};
use crate::event::{Common, Event, Harness, Tool, ToolInput, ToolResponse};
use crate::evidence::{outcome, runner};
use crate::ledger::{self, Line};
use crate::output;
use crate::repo;
use crate::rules;
use crate::rules::policy;

pub struct HookOutcome {
    pub stdout: Option<String>,
}

const NONE: HookOutcome = HookOutcome { stdout: None };

pub fn run(harness: Harness, stdin_json: &str) -> Result<HookOutcome, String> {
    let raw: serde_json::Value = serde_json::from_str(stdin_json).map_err(|e| format!("bad json: {e}"))?;
    let event = adapters::parse(harness, &raw)?;
    let root = repo::find_root(&event.common().cwd);
    Ok(match event {
        Event::PreToolUse {
            common,
            tool,
            tool_use_id,
            input,
        } => on_pre_tool(&root, &common, &tool, tool_use_id, &input)?,
        Event::PostToolUse {
            common,
            tool,
            tool_use_id,
            input,
            response,
        } => {
            on_post_tool(&root, &common, &tool, tool_use_id, &input, &response)?;
            NONE
        }
        Event::Stop {
            common, last_message, ..
        } => on_stop(&root, &common, &last_message)?,
        Event::PreCompact { common, .. } => {
            on_pre_compact(&root, &common)?;
            NONE
        }
        _ => NONE,
    })
}

fn line_agent(l: &Line) -> Option<&String> {
    match l {
        Line::Run { agent_id, .. }
        | Line::EditPending { agent_id, .. }
        | Line::Edit { agent_id, .. }
        | Line::Claim { agent_id, .. }
        | Line::Decision { agent_id, .. } => agent_id.as_ref(),
        Line::Snapshot { .. } | Line::Override { .. } => None,
    }
}

/// The main agent sees every line; a subagent sees its own lines plus unscoped ones.
///
/// Spec §8 asks for strict per-agent isolation. Deviation: whether the harnesses stamp
/// `agent_id` on tool payloads is unverified, so scoping a subagent to its own lines only
/// would drop its real evidence and block honest work. Unscoped lines are therefore shared.
/// Tighten in v1 once the stamping is confirmed.
pub fn scoped(lines: Vec<Line>, agent_id: &Option<String>) -> Vec<Line> {
    match agent_id {
        None => lines,
        Some(id) => lines
            .into_iter()
            .filter(|l| line_agent(l).map(|a| a == id).unwrap_or(true))
            .collect(),
    }
}

fn on_pre_compact(root: &Path, common: &Common) -> Result<(), String> {
    let lines = ledger::read(root, &common.session_id);
    let count = |f: &dyn Fn(&Line) -> bool| lines.iter().filter(|l| f(l)).count() as u32;
    let snapshot = Line::Snapshot {
        ts: ledger::now_ms(),
        runs: count(&|l| matches!(l, Line::Run { .. })),
        passes: count(&|l| matches!(l, Line::Run { outcome, .. } if outcome == "pass")),
        fails: count(&|l| matches!(l, Line::Run { outcome, .. } if outcome == "fail")),
        edits_changed: count(&|l| matches!(l, Line::Edit { changed: true, .. })),
        claims: count(&|l| matches!(l, Line::Claim { .. })),
    };
    ledger::append(root, &common.session_id, &snapshot)
}

/// Files an edit tool touches: `file_path`, or the paths named in an apply_patch body.
///
/// Relative paths are joined against the event's `cwd`, not the repo root: Codex
/// `apply_patch` entries are relative to the process cwd, which is a subdirectory
/// whenever the agent runs below the repo root.
pub fn edit_paths(cwd: &Path, input: &ToolInput) -> Vec<PathBuf> {
    if let Some(p) = &input.file_path {
        return vec![if p.is_absolute() { p.clone() } else { cwd.join(p) }];
    }
    let Some(patch) = &input.patch else {
        return Vec::new();
    };
    patch
        .lines()
        .filter_map(|l| {
            l.strip_prefix("*** Update File: ")
                .or_else(|| l.strip_prefix("*** Add File: "))
                .or_else(|| l.strip_prefix("*** Delete File: "))
        })
        .map(|p| cwd.join(p.trim()))
        .collect()
}

/// Path-like tokens a shell command names (`sed -i … src/app.py`, `cat > notes/new.md`, a
/// `pathlib.Path('npm/install.js')` inside a heredoc). Bash gives no structured file list, so this
/// is how a Bash-made edit earns an `edit` line: hash before, hash after, record when changed.
/// Kept: existing files, or not-yet-existing files whose parent directory exists. Capped at 32.
pub fn command_paths(cwd: &Path, command: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for tok in command.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '.' | '/' | '-' | '~'))) {
        let t = tok.trim_matches(|c: char| matches!(c, '.' | '-'));
        let looks_like_path = t.contains('/')
            || t.rsplit_once('.').is_some_and(|(stem, ext)| {
                !stem.is_empty()
                    && !ext.is_empty()
                    && ext.chars().all(|c| c.is_ascii_alphanumeric())
                    && !ext.chars().all(|c| c.is_ascii_digit())
            });
        if t.is_empty() || t.starts_with('~') || t.ends_with('/') || !looks_like_path || tok.contains("://") {
            continue;
        }
        let p = if Path::new(t).is_absolute() {
            PathBuf::from(t)
        } else {
            cwd.join(t)
        };
        let written_to = t.contains('/')
            || [format!("> {t}"), format!(">{t}"), format!("-o {t}")]
                .iter()
                .any(|w| command.contains(w));
        let keep = p.is_file() || (!p.exists() && written_to && p.parent().is_some_and(|d| d.is_dir()));
        if keep && !out.contains(&p) {
            out.push(p);
            if out.len() == 32 {
                break;
            }
        }
    }
    out
}

fn on_pre_tool(
    root: &Path,
    common: &Common,
    tool: &Tool,
    tool_use_id: Option<String>,
    input: &ToolInput,
) -> Result<HookOutcome, String> {
    let lines = ledger::read(root, &common.session_id);
    let pol = policy::load(root);
    let mut blocks = Vec::new();
    if !rules::disabled("R3") {
        if *tool == Tool::Bash {
            blocks.extend(policy::check_command(
                &pol,
                &input.command.clone().unwrap_or_default(),
                &lines,
            ));
        } else if tool.is_edit() {
            for p in edit_paths(&common.cwd, input) {
                blocks.extend(policy::check_edit(&pol, &repo::rel(root, &p)));
            }
        }
    }
    if !blocks.is_empty() {
        let out = record_and_answer(root, common, &lines, blocks, output::pre_tool_deny, false)?;
        if out.stdout.is_some() {
            return Ok(out);
        }
    }
    let touched = if *tool == Tool::Bash {
        command_paths(&common.cwd, &input.command.clone().unwrap_or_default())
    } else if tool.is_edit() {
        edit_paths(&common.cwd, input)
    } else {
        Vec::new()
    };
    {
        for p in touched {
            ledger::append(
                root,
                &common.session_id,
                &Line::EditPending {
                    ts: ledger::now_ms(),
                    agent_id: common.agent_id.clone(),
                    tool_use_id: tool_use_id.clone(),
                    path: repo::rel(root, &p),
                    hash_before: repo::sha256_file(&p),
                },
            )?;
        }
    }
    Ok(NONE)
}

fn on_stop(root: &Path, common: &Common, last_message: &str) -> Result<HookOutcome, String> {
    let lines = scoped(ledger::read(root, &common.session_id), &common.agent_id);
    let found = claims::extract(last_message, root);
    for c in &found {
        ledger::append(
            root,
            &common.session_id,
            &Line::Claim {
                ts: ledger::now_ms(),
                agent_id: common.agent_id.clone(),
                event: common.event_name.clone(),
                class: c.class.as_str().to_string(),
                text: c.text.clone(),
                path: c.path.clone(),
            },
        )?;
    }
    let mut blocks = Vec::new();
    if !rules::disabled(rules::r1_tests::ID) {
        blocks.extend(rules::r1_tests::evaluate(&lines, &found));
    }
    if !rules::disabled(rules::r2_edit::ID) {
        blocks.extend(rules::r2_edit::evaluate(&lines, &found));
    }
    let out = record_and_answer(root, common, &lines, blocks, output::stop_block, true)?;
    let _ = crate::report::write(root, &common.session_id);
    Ok(out)
}

fn decision_line(common: &Common, decision: &str, rule: &str, reason: &str, consecutive: u32) -> Line {
    Line::Decision {
        ts: ledger::now_ms(),
        agent_id: common.agent_id.clone(),
        event: common.event_name.clone(),
        decision: decision.into(),
        rule: rule.into(),
        reason: reason.into(),
        consecutive,
    }
}

fn record_and_answer(
    root: &Path,
    common: &Common,
    lines: &[Line],
    blocks: Vec<rules::Block>,
    render: fn(&str) -> String,
    cap: bool,
) -> Result<HookOutcome, String> {
    let line;
    let out = match decide::verdict(lines, blocks, cap) {
        Verdict::Allow => {
            line = decision_line(common, "allow", "", "", 0);
            NONE
        }
        Verdict::Block {
            rule,
            reason,
            consecutive,
        } => {
            line = decision_line(common, "block", rule, &reason, consecutive);
            HookOutcome {
                stdout: Some(render(&reason)),
            }
        }
        Verdict::Capped { rule } => {
            line = decision_line(
                common,
                "capped",
                rule,
                "consecutive block cap reached; allowing",
                0,
            );
            NONE
        }
        Verdict::Overridden => {
            line = decision_line(common, "override", "", "human override consumed", 0);
            NONE
        }
    };
    ledger::append(root, &common.session_id, &line)?;
    Ok(out)
}

/// The `edit_pending` entry for `path` (matching `tool_use_id` when given): outer `None` means no
/// entry was recorded; inner `None` means the file did not exist beforehand.
fn pending_before(lines: &[Line], tool_use_id: &Option<String>, path: &str) -> Option<Option<String>> {
    lines.iter().rev().find_map(|l| match l {
        Line::EditPending {
            tool_use_id: id,
            path: p,
            hash_before,
            ..
        } if p == path && (tool_use_id.is_none() || id == tool_use_id) => Some(hash_before.clone()),
        _ => None,
    })
}

fn pending_hash(lines: &[Line], tool_use_id: &Option<String>, path: &str) -> Option<String> {
    pending_before(lines, tool_use_id, path)?
}

fn on_post_tool(
    root: &Path,
    common: &Common,
    tool: &Tool,
    tool_use_id: Option<String>,
    input: &ToolInput,
    response: &ToolResponse,
) -> Result<(), String> {
    if *tool == Tool::Bash {
        let command = input.command.clone().unwrap_or_default();
        let r = runner::classify(&command);
        let o = outcome::infer(
            r,
            &response.stdout,
            &response.stderr,
            response.is_error,
            response.interrupted,
        );
        ledger::append(
            root,
            &common.session_id,
            &Line::Run {
                ts: ledger::now_ms(),
                agent_id: common.agent_id.clone(),
                tool: tool.as_str(),
                command,
                runner: r.as_str().to_string(),
                outcome: o.as_str().to_string(),
                stdout_hash: repo::sha256_str(&response.stdout),
                stderr_hash: repo::sha256_str(&response.stderr),
                is_error: response.is_error,
                interrupted: response.interrupted,
            },
        )?;
    }
    if *tool == Tool::Bash {
        let command = input.command.clone().unwrap_or_default();
        let paths = command_paths(&common.cwd, &command);
        if !paths.is_empty() {
            let lines = ledger::read(root, &common.session_id);
            for p in paths {
                let path = repo::rel(root, &p);
                let Some(hash_before) = pending_before(&lines, &tool_use_id, &path) else {
                    continue;
                };
                let hash_after = repo::sha256_file(&p);
                if hash_before == hash_after {
                    continue;
                }
                ledger::append(
                    root,
                    &common.session_id,
                    &Line::Edit {
                        ts: ledger::now_ms(),
                        agent_id: common.agent_id.clone(),
                        tool: tool.as_str(),
                        path,
                        hash_before,
                        hash_after,
                        changed: true,
                    },
                )?;
            }
        }
    }
    if tool.is_edit() {
        let lines = ledger::read(root, &common.session_id);
        for p in edit_paths(&common.cwd, input) {
            let path = repo::rel(root, &p);
            let hash_before = pending_hash(&lines, &tool_use_id, &path);
            let hash_after = repo::sha256_file(&p);
            let changed = hash_before != hash_after;
            ledger::append(
                root,
                &common.session_id,
                &Line::Edit {
                    ts: ledger::now_ms(),
                    agent_id: common.agent_id.clone(),
                    tool: tool.as_str(),
                    path,
                    hash_before,
                    hash_after,
                    changed,
                },
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_paths_keeps_named_files_and_new_files_with_a_parent() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        std::fs::create_dir_all(cwd.join("src")).unwrap();
        std::fs::create_dir_all(cwd.join("notes")).unwrap();
        std::fs::write(cwd.join("src/app.py"), "x").unwrap();
        let cmd = "FOO=1 sed -i '' 's/1/2/' src/app.py && cat > notes/new.md <<'EOF'\nhi\nEOF\n\
                   python3 - <<'PY'\nimport pathlib; pathlib.Path('src/app.py').write_text('y')\nPY\n\
                   curl https://example.com/x.tar.gz -o /nonexistent-dir/x.tar.gz; ls src/; echo 1.23 v2.0 ./src/app.py; echo hi > new.md";
        let got = command_paths(cwd, cmd);
        assert_eq!(
            got,
            vec![
                cwd.join("src/app.py"),
                cwd.join("notes/new.md"),
                cwd.join("new.md")
            ],
            "{got:?}"
        );
        assert!(command_paths(cwd, "cargo test --workspace && git commit -m 'wip'").is_empty());
    }
}
