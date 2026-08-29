use std::path::PathBuf;

use serde_json::Value;

use crate::event::{Common, Event, Harness, Tool, ToolInput, ToolResponse};

pub fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(|x| x.to_string())
}

fn common(raw: &Value, harness: Harness) -> Result<Common, String> {
    let session_id = s(raw, "session_id").ok_or("missing session_id")?;
    let cwd = s(raw, "cwd").map(PathBuf::from).ok_or("missing cwd")?;
    let event_name = s(raw, "hook_event_name").ok_or("missing hook_event_name")?;
    Ok(Common {
        harness,
        session_id,
        cwd,
        event_name,
        agent_id: s(raw, "agent_id"),
    })
}

pub fn tool_from_name(name: &str) -> Tool {
    match name {
        "Bash" | "shell" => Tool::Bash,
        "Edit" => Tool::Edit,
        "Write" => Tool::Write,
        "MultiEdit" => Tool::MultiEdit,
        "NotebookEdit" => Tool::NotebookEdit,
        "apply_patch" => Tool::ApplyPatch,
        other => Tool::Other(other.to_string()),
    }
}

fn input(raw: &Value) -> ToolInput {
    let ti = raw.get("tool_input").cloned().unwrap_or(Value::Null);
    ToolInput {
        command: s(&ti, "command"),
        file_path: s(&ti, "file_path")
            .or_else(|| s(&ti, "notebook_path"))
            .map(PathBuf::from),
        patch: s(&ti, "patch").or_else(|| s(&ti, "input")),
    }
}

pub fn response(raw: &Value) -> ToolResponse {
    let tr = raw.get("tool_response").cloned().unwrap_or(Value::Null);
    match &tr {
        Value::String(text) => ToolResponse {
            stdout: text.clone(),
            ..Default::default()
        },
        Value::Object(_) => ToolResponse {
            stdout: s(&tr, "stdout").or_else(|| s(&tr, "output")).unwrap_or_default(),
            stderr: s(&tr, "stderr").unwrap_or_default(),
            is_error: tr.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false),
            interrupted: tr.get("interrupted").and_then(|b| b.as_bool()).unwrap_or(false),
        },
        _ => ToolResponse::default(),
    }
}

pub fn parse(raw: &Value) -> Result<Event, String> {
    parse_with(raw, Harness::Claude)
}

pub(crate) fn parse_with(raw: &Value, harness: Harness) -> Result<Event, String> {
    let common = common(raw, harness)?;
    let name = common.event_name.clone();
    let tool_use_id = s(raw, "tool_use_id");
    let tool = tool_from_name(&s(raw, "tool_name").unwrap_or_default());
    Ok(match name.as_str() {
        "PreToolUse" => Event::PreToolUse {
            tool,
            tool_use_id,
            input: input(raw),
            common,
        },
        "PostToolUse" => Event::PostToolUse {
            tool,
            tool_use_id,
            input: input(raw),
            response: response(raw),
            common,
        },
        "Stop" | "SubagentStop" => Event::Stop {
            last_message: s(raw, "last_assistant_message").unwrap_or_default(),
            stop_hook_active: raw
                .get("stop_hook_active")
                .and_then(|b| b.as_bool())
                .unwrap_or(false),
            common,
        },
        "PreCompact" => Event::PreCompact {
            trigger: s(raw, "trigger").unwrap_or_default(),
            common,
        },
        _ => Event::Other { common },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, Tool};

    #[test]
    fn parses_stop_with_last_message() {
        let raw: serde_json::Value =
            serde_json::from_str(include_str!("../../fixtures/hooks/claude/stop-lying.json")).unwrap();
        let ev = parse(&raw).unwrap();
        match ev {
            Event::Stop {
                last_message,
                stop_hook_active,
                common,
            } => {
                assert!(last_message.contains("All tests pass"));
                assert!(!stop_hook_active);
                assert_eq!(common.session_id, "sess-lying");
                assert_eq!(common.event_name, "Stop");
                assert_eq!(common.agent_id, None);
            }
            other => panic!("wrong event: {other:?}"),
        }
    }

    fn load(name: &str) -> Event {
        let path = format!("{}/fixtures/hooks/claude/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("__CWD__", "/tmp/proj");
        parse(&serde_json::from_str(&text).unwrap()).unwrap()
    }

    #[test]
    fn parses_pre_and_post_bash() {
        match load("pre-bash.json") {
            Event::PreToolUse {
                tool,
                tool_use_id,
                input,
                ..
            } => {
                assert_eq!(tool, Tool::Bash);
                assert_eq!(tool_use_id.as_deref(), Some("toolu_b1"));
                assert_eq!(input.command.as_deref(), Some("pytest -q"));
            }
            e => panic!("{e:?}"),
        }
        match load("post-bash-pytest-fail.json") {
            Event::PostToolUse { response, .. } => {
                assert!(response.stdout.contains("1 failed"));
                assert!(response.is_error);
                assert!(!response.interrupted);
            }
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn parses_edit_and_write_paths() {
        match load("pre-edit.json") {
            Event::PreToolUse { tool, input, .. } => {
                assert_eq!(tool, Tool::Edit);
                assert_eq!(input.file_path.unwrap().to_str().unwrap(), "/tmp/proj/src/app.py");
            }
            e => panic!("{e:?}"),
        }
        match load("post-write.json") {
            Event::PostToolUse { tool, input, .. } => {
                assert_eq!(tool, Tool::Write);
                assert!(input.file_path.unwrap().ends_with("migrations/0002_add.sql"));
            }
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn subagent_stop_carries_agent_id_and_pre_compact_trigger() {
        match load("subagent-stop-lying.json") {
            Event::Stop { common, .. } => {
                assert_eq!(common.event_name, "SubagentStop");
                assert_eq!(common.agent_id.as_deref(), Some("agent-7"));
            }
            e => panic!("{e:?}"),
        }
        match load("pre-compact.json") {
            Event::PreCompact { trigger, .. } => assert_eq!(trigger, "auto"),
            e => panic!("{e:?}"),
        }
    }
}
