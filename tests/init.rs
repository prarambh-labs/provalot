mod common;

#[test]
fn project_init_writes_both_configs_and_gitignore() {
    let dir = tempfile::tempdir().unwrap();
    let out = common::run(&["init"], "", dir.path(), &[]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let claude: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap())
            .unwrap();
    let claude_cmd = claude["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        claude_cmd.ends_with("provalot hook claude"),
        "command is {claude_cmd}"
    );
    assert!(
        claude_cmd.starts_with('/'),
        "the hook command is an absolute path, not a bare name: {claude_cmd}"
    );
    let codex: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join(".codex/hooks.json")).unwrap())
            .unwrap();
    assert_eq!(codex["hooks"]["PreToolUse"][0]["matcher"], "Bash|apply_patch");
    let codex_cmd = codex["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        codex_cmd.ends_with("provalot hook codex"),
        "command is {codex_cmd}"
    );
    assert!(std::fs::read_to_string(dir.path().join(".gitignore"))
        .unwrap()
        .contains(".provalot/"));
    assert!(
        common::stdout(&out).contains("/hooks"),
        "codex trust step is printed"
    );
}

#[test]
fn user_init_writes_home_settings_and_uninstall_removes_them() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let env = [("HOME", home.path().to_str().unwrap())];
    common::run(&["init", "--claude", "--user"], "", dir.path(), &env);
    let p = home.path().join(".claude/settings.json");
    let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    assert!(s["hooks"]["PreCompact"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .ends_with("provalot hook claude"));
    common::run(&["uninstall", "--claude", "--user"], "", dir.path(), &env);
    let s: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    assert!(s["hooks"].as_object().map(|o| o.is_empty()).unwrap_or(true));
}

#[test]
fn init_preserves_existing_entries() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(dir.path().join(".claude/settings.json"), r#"{"permissions":{"allow":["Bash(ls *)"]},"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"rtk hook claude"}]}]}}"#).unwrap();
    common::run(&["init", "--claude"], "", dir.path(), &[]);
    let s: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap())
            .unwrap();
    assert_eq!(s["permissions"]["allow"][0], "Bash(ls *)");
    assert_eq!(
        s["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "rtk hook claude"
    );
    assert!(s["hooks"]["PreToolUse"][1]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .ends_with("provalot hook claude"));
}

/// `uninstall` must clean up both the bare form (plugin / older installs) and the
/// absolute-path form that `init` writes now.
#[test]
fn uninstall_removes_bare_and_absolute_hook_commands() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(
        dir.path().join(".claude/settings.json"),
        r#"{"hooks":{"Stop":[
          {"hooks":[{"type":"command","command":"provalot hook claude"}]},
          {"hooks":[{"type":"command","command":"/opt/bin/provalot hook claude"}]},
          {"hooks":[{"type":"command","command":"rtk hook claude"}]}
        ]}}"#,
    )
    .unwrap();
    common::run(&["init", "--claude"], "", dir.path(), &[]);
    common::run(&["uninstall", "--claude"], "", dir.path(), &[]);
    let s: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap())
            .unwrap();
    let stop = s["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 1, "only the foreign hook survives: {stop:?}");
    assert_eq!(stop[0]["hooks"][0]["command"], "rtk hook claude");
}

/// Installing from a second path (a fresh `npx` cache, then a global install) must leave one
/// hook entry per event, carrying the newest path.
#[test]
fn installing_from_two_paths_leaves_one_entry_per_event() {
    let dir = tempfile::tempdir().unwrap();
    let bins = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for name in ["a", "b"] {
        let d = bins.path().join(name);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("provalot");
        std::fs::copy(env!("CARGO_BIN_EXE_provalot"), &p).unwrap();
        paths.push(p);
    }
    for p in &paths {
        let out = std::process::Command::new(p)
            .args(["init", "--claude"])
            .current_dir(dir.path())
            .output()
            .expect("run init");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let s: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap())
            .unwrap();
    for ev in ["PreToolUse", "PostToolUse", "Stop", "SubagentStop", "PreCompact"] {
        let list = s["hooks"][ev].as_array().unwrap();
        assert_eq!(list.len(), 1, "{ev} has one entry: {list:?}");
        assert_eq!(
            list[0]["hooks"][0]["command"],
            format!("{} hook claude", paths[1].display()),
            "{ev} carries the newest install path"
        );
    }
}
