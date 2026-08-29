use std::path::{Path, PathBuf};

use crate::adapters;
use crate::claims;
use crate::event::{Common, Event, Harness, Tool, ToolInput, ToolResponse};
use crate::evidence::{outcome, runner};
use crate::ledger::{self, Line};
use crate::output;
use crate::repo;

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
        } => {
            let found = claims::extract(&last_message, &root);
            if found.iter().any(|c| c.class == claims::ClaimClass::TestsPass) {
                HookOutcome {
                    stdout: Some(output::stop_block(
                        "[provalot] Claimed tests pass, but no test runner has passed in this session. Run the tests now, or say they were not run.",
                    )),
                }
            } else {
                let _ = common;
                NONE
            }
        }
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
