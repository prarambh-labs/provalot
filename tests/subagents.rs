mod common;

fn hook(dir: &std::path::Path, fixture: &str) -> String {
    common::stdout(&common::run(
        &["hook", "claude"],
        &common::fixture(fixture, dir),
        dir,
        &[],
    ))
}

#[test]
fn another_agents_run_does_not_back_this_subagents_claim() {
    let dir = tempfile::tempdir().unwrap();
    hook(dir.path(), "claude/post-bash-pytest-pass-agent9.json");
    assert!(hook(dir.path(), "claude/subagent-stop-lying.json").contains("\"block\""));
}

#[test]
fn unscoped_run_backs_a_subagent_claim_and_the_main_agent_sees_everything() {
    let dir = tempfile::tempdir().unwrap();
    hook(dir.path(), "claude/post-bash-pytest-pass.json");
    assert_eq!(hook(dir.path(), "claude/subagent-stop-lying.json"), "");
    let dir2 = tempfile::tempdir().unwrap();
    hook(dir2.path(), "claude/post-bash-pytest-pass-agent9.json");
    assert_eq!(
        hook(dir2.path(), "claude/stop-tests-only.json"),
        "",
        "main Stop counts subagent evidence"
    );
}

#[test]
fn pre_compact_writes_a_snapshot_of_counts() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/app.py"), "print(1)\n").unwrap();
    hook(dir.path(), "claude/post-bash-pytest-pass.json");
    hook(dir.path(), "claude/post-bash-pytest-fail.json");
    hook(dir.path(), "claude/pre-edit.json");
    std::fs::write(dir.path().join("src/app.py"), "print(2)\n").unwrap();
    hook(dir.path(), "claude/post-edit.json");
    assert_eq!(hook(dir.path(), "claude/pre-compact.json"), "");
    let last = common::ledger_lines(dir.path(), "sess-1").pop().unwrap();
    assert_eq!(last["kind"], "snapshot");
    assert_eq!(last["runs"], 2);
    assert_eq!(last["passes"], 1);
    assert_eq!(last["fails"], 1);
    assert_eq!(last["edits_changed"], 1);
}
