use serde_json::json;

pub fn stop_block(reason: &str) -> String {
    json!({"decision": "block", "reason": reason}).to_string()
}

pub fn pre_tool_deny(reason: &str) -> String {
    json!({"hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": reason
    }})
    .to_string()
}
