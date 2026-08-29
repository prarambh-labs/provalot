use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub const CLAUDE_COMMAND: &str = "provalot hook claude";
pub const CODEX_COMMAND: &str = "provalot hook codex";
pub const CLAUDE_MATCHER: &str = "Bash|Edit|Write|MultiEdit|NotebookEdit";
pub const CODEX_MATCHER: &str = "Bash|apply_patch";
const TOOL_EVENTS: [&str; 2] = ["PreToolUse", "PostToolUse"];
const PLAIN_EVENTS: [&str; 3] = ["Stop", "SubagentStop", "PreCompact"];
pub const CODEX_TRUST_NOTE: &str =
    "Codex: project hooks load only after you trust them. In Codex run /hooks, review 'provalot hook codex', and trust it.";

/// The bare command with `provalot` replaced by this binary's absolute path.
///
/// After `npx provalot init` exits, nothing named `provalot` is on PATH, so a bare
/// command would make every hook invocation fail with "command not found". Falls back
/// to the bare form when the executable path cannot be resolved. `hooks/hooks.json`
/// stays bare: a plugin cannot know where the binary lives.
pub fn resolved_command(bare: &str) -> String {
    let Ok(exe) = std::env::current_exe() else {
        return bare.to_string();
    };
    let Some(args) = bare.strip_prefix("provalot") else {
        return bare.to_string();
    };
    let exe = exe.to_string_lossy();
    if exe.contains(char::is_whitespace) {
        format!("\"{exe}\"{args}")
    } else {
        format!("{exe}{args}")
    }
}

pub fn hook_entry(command: &str, matcher: Option<&str>) -> Value {
    let mut entry = json!({"hooks": [{"type": "command", "command": command, "timeout": 10}]});
    if let Some(m) = matcher {
        entry["matcher"] = json!(m);
    }
    entry
}

fn has_command(entry: &Value, command: &str) -> bool {
    entry["hooks"]
        .as_array()
        .map(|hs| hs.iter().any(|h| h["command"].as_str() == Some(command)))
        .unwrap_or(false)
}

pub fn add_hooks(settings: &mut Value, command: &str, matcher: &str) -> bool {
    if !settings.is_object() {
        *settings = json!({});
    }
    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert(json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let mut changed = false;
    let events = TOOL_EVENTS
        .iter()
        .map(|e| (*e, Some(matcher)))
        .chain(PLAIN_EVENTS.iter().map(|e| (*e, None)));
    for (ev, m) in events {
        let arr = hooks.as_object_mut().unwrap().entry(ev).or_insert(json!([]));
        if !arr.is_array() {
            *arr = json!([]);
        }
        let list = arr.as_array_mut().unwrap();
        if !list.iter().any(|e| has_command(e, command)) {
            list.push(hook_entry(command, m));
            changed = true;
        }
    }
    changed
}

/// True for a hook command we installed, in either the bare or the absolute-path form.
fn is_ours(command: &str) -> bool {
    command.contains("provalot") && command.contains(" hook ")
}

pub fn remove_hooks(settings: &mut Value) -> bool {
    let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) else {
        return false;
    };
    let mut changed = false;
    let keys: Vec<String> = hooks.keys().cloned().collect();
    for k in keys {
        let Some(list) = hooks.get_mut(&k).and_then(|a| a.as_array_mut()) else {
            continue;
        };
        let before = list.len();
        list.retain(|e| {
            !e["hooks"]
                .as_array()
                .map(|hs| {
                    hs.iter()
                        .any(|h| h["command"].as_str().map(is_ours).unwrap_or(false))
                })
                .unwrap_or(false)
        });
        if list.len() != before {
            changed = true;
        }
        if list.is_empty() {
            hooks.remove(&k);
        }
    }
    changed
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn claude_settings_path(root: &Path, user: bool) -> PathBuf {
    if user {
        home().join(".claude/settings.json")
    } else {
        root.join(".claude/settings.json")
    }
}

pub fn codex_hooks_path(root: &Path, user: bool) -> PathBuf {
    if user {
        home().join(".codex/hooks.json")
    } else {
        root.join(".codex/hooks.json")
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => {
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
        }
        _ => Ok(json!({})),
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

pub fn ensure_gitignore(root: &Path) -> Result<bool, String> {
    let p = root.join(".gitignore");
    let existing = std::fs::read_to_string(&p).unwrap_or_default();
    if existing.lines().any(|l| {
        matches!(
            l.trim(),
            ".provalot" | ".provalot/" | "/.provalot" | "/.provalot/"
        )
    }) {
        return Ok(false);
    }
    let sep = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    std::fs::write(&p, format!("{existing}{sep}.provalot/\n")).map_err(|e| e.to_string())?;
    Ok(true)
}

fn apply(
    path: &Path,
    f: impl Fn(&mut Value) -> bool,
    verb: &str,
    msgs: &mut Vec<String>,
) -> Result<(), String> {
    let mut v = read_json(path)?;
    if f(&mut v) {
        write_json(path, &v)?;
        msgs.push(format!("{verb} {}", path.display()));
    } else {
        msgs.push(format!("unchanged {}", path.display()));
    }
    Ok(())
}

pub fn init(root: &Path, claude: bool, codex: bool, user: bool) -> Result<Vec<String>, String> {
    let (claude, codex) = if !claude && !codex {
        (true, true)
    } else {
        (claude, codex)
    };
    let mut msgs = Vec::new();
    if claude {
        apply(
            &claude_settings_path(root, user),
            |v| add_hooks(v, &resolved_command(CLAUDE_COMMAND), CLAUDE_MATCHER),
            "wrote",
            &mut msgs,
        )?;
    }
    if codex {
        apply(
            &codex_hooks_path(root, user),
            |v| add_hooks(v, &resolved_command(CODEX_COMMAND), CODEX_MATCHER),
            "wrote",
            &mut msgs,
        )?;
        msgs.push(CODEX_TRUST_NOTE.to_string());
    }
    if ensure_gitignore(root)? {
        msgs.push(format!(
            "added .provalot/ to {}",
            root.join(".gitignore").display()
        ));
    }
    Ok(msgs)
}

pub fn uninstall(root: &Path, claude: bool, codex: bool, user: bool) -> Result<Vec<String>, String> {
    let (claude, codex) = if !claude && !codex {
        (true, true)
    } else {
        (claude, codex)
    };
    let mut msgs = Vec::new();
    if claude {
        apply(
            &claude_settings_path(root, user),
            remove_hooks,
            "cleaned",
            &mut msgs,
        )?;
    }
    if codex {
        apply(&codex_hooks_path(root, user), remove_hooks, "cleaned", &mut msgs)?;
    }
    Ok(msgs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn adds_five_events_once_and_keeps_other_hooks() {
        let mut s = json!({"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "rtk hook claude"}]}]}});
        assert!(add_hooks(&mut s, CLAUDE_COMMAND, CLAUDE_MATCHER));
        assert!(!add_hooks(&mut s, CLAUDE_COMMAND, CLAUDE_MATCHER), "idempotent");
        let pre = s["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0]["hooks"][0]["command"], "rtk hook claude");
        assert_eq!(pre[1]["matcher"], CLAUDE_MATCHER);
        assert_eq!(pre[1]["hooks"][0]["command"], CLAUDE_COMMAND);
        assert_eq!(pre[1]["hooks"][0]["timeout"], 10);
        for ev in ["PostToolUse", "Stop", "SubagentStop", "PreCompact"] {
            assert_eq!(s["hooks"][ev].as_array().unwrap().len(), 1, "{ev}");
        }
        assert!(s["hooks"]["Stop"][0].get("matcher").is_none());
    }

    #[test]
    fn removes_only_our_entries() {
        let mut s = json!({});
        add_hooks(&mut s, CLAUDE_COMMAND, CLAUDE_MATCHER);
        s["hooks"]["PreToolUse"].as_array_mut().unwrap().insert(
            0,
            json!({"matcher": "Bash", "hooks": [{"type": "command", "command": "rtk hook claude"}]}),
        );
        s["hooks"]["PreToolUse"]
            .as_array_mut()
            .unwrap()
            .push(json!({"hooks": [{"type": "command", "command": "/opt/bin/provalot hook claude"}]}));
        assert!(remove_hooks(&mut s));
        assert_eq!(s["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert!(s["hooks"].get("Stop").is_none(), "empty arrays are dropped");
        assert!(!remove_hooks(&mut s));
    }

    #[test]
    fn gitignore_accepts_every_spelling_of_the_entry() {
        for line in [".provalot", ".provalot/", "/.provalot", "/.provalot/"] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join(".gitignore"), format!("target\n{line}\n")).unwrap();
            assert!(
                !ensure_gitignore(dir.path()).unwrap(),
                "{line} is already ignored"
            );
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target").unwrap();
        assert!(ensure_gitignore(dir.path()).unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            "target\n.provalot/\n"
        );
    }
}
