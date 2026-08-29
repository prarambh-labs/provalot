use serde_json::Value;

use crate::event::{Event, Harness};

/// Codex CLI hook payloads share Claude Code's field names; the differences are the tool names
/// (`shell`/`Bash`, `apply_patch`), string-shaped `tool_response`, and the `output` field.
pub fn parse(raw: &Value) -> Result<Event, String> {
    super::claude::parse_with(raw, Harness::Codex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, Harness, Tool};

    fn load(name: &str) -> Event {
        let path = format!("{}/fixtures/hooks/codex/{name}", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("__CWD__", "/tmp/proj");
        parse(&serde_json::from_str(&text).unwrap()).unwrap()
    }

    #[test]
    fn string_tool_response_and_harness_tag() {
        match load("post-bash-cargo-pass.json") {
            Event::PostToolUse {
                common,
                tool,
                response,
                ..
            } => {
                assert_eq!(common.harness, Harness::Codex);
                assert_eq!(tool, Tool::Bash);
                assert!(response.stdout.contains("test result: ok"));
            }
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn apply_patch_carries_the_patch_and_output_field() {
        match load("pre-apply-patch.json") {
            Event::PreToolUse { tool, input, .. } => {
                assert_eq!(tool, Tool::ApplyPatch);
                assert!(input.patch.unwrap().contains("*** Update File: src/app.py"));
            }
            e => panic!("{e:?}"),
        }
        match load("post-apply-patch.json") {
            Event::PostToolUse { response, .. } => assert!(response.stdout.contains("Success")),
            e => panic!("{e:?}"),
        }
    }
}
