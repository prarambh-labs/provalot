mod common;

#[test]
fn project_init_writes_both_configs_and_gitignore() {
    let dir = tempfile::tempdir().unwrap();
    let out = common::run(&["init"], "", dir.path(), &[]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let claude: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        claude["hooks"]["Stop"][0]["hooks"][0]["command"],
        "provalot hook claude"
    );
    let codex: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join(".codex/hooks.json")).unwrap())
            .unwrap();
    assert_eq!(codex["hooks"]["PreToolUse"][0]["matcher"], "Bash|apply_patch");
    assert_eq!(
        codex["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "provalot hook codex"
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
    assert_eq!(
        s["hooks"]["PreCompact"][0]["hooks"][0]["command"],
        "provalot hook claude"
    );
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
    assert_eq!(
        s["hooks"]["PreToolUse"][1]["hooks"][0]["command"],
        "provalot hook claude"
    );
}
