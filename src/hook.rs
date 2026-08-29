use std::path::Path;

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
        Event::Stop { last_message, .. } if claims::has_tests_pass_claim(&last_message) => HookOutcome {
            stdout: Some(output::stop_block(
                "[provalot] Claimed tests pass, but no test runner has passed in this session. Run the tests now, or say they were not run.",
            )),
        },
        _ => NONE,
    })
}

fn on_post_tool(
    root: &Path,
    common: &Common,
    tool: &Tool,
    _tool_use_id: Option<String>,
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
    Ok(())
}
