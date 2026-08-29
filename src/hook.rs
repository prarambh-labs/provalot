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
        _ => NONE,
    })
}

/// Files an edit tool touches: `file_path`, or the paths named in an apply_patch body.
pub fn edit_paths(root: &Path, input: &ToolInput) -> Vec<PathBuf> {
    if let Some(p) = &input.file_path {
        return vec![if p.is_absolute() { p.clone() } else { root.join(p) }];
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
        .map(|p| root.join(p.trim()))
        .collect()
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
            for p in edit_paths(root, input) {
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
    if tool.is_edit() {
        for p in edit_paths(root, input) {
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
    let lines = ledger::read(root, &common.session_id);
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
    record_and_answer(root, common, &lines, blocks, output::stop_block, true)
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

fn pending_hash(lines: &[Line], tool_use_id: &Option<String>, path: &str) -> Option<String> {
    lines.iter().rev().find_map(|l| match l {
        Line::EditPending {
            tool_use_id: id,
            path: p,
            hash_before,
            ..
        } if p == path && (tool_use_id.is_none() || id == tool_use_id) => Some(hash_before.clone()),
        _ => None,
    })?
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
    if tool.is_edit() {
        let lines = ledger::read(root, &common.session_id);
        for p in edit_paths(root, input) {
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
