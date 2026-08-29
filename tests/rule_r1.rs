mod common;

fn hook(dir: &std::path::Path, fixture: &str) -> std::process::Output {
    common::run(&["hook", "claude"], &common::fixture(fixture, dir), dir, &[])
}

#[test]
fn truthful_stop_after_passing_run_is_allowed() {
    let dir = tempfile::tempdir().unwrap();
    hook(dir.path(), "claude/post-bash-pytest-pass.json");
    let out = hook(dir.path(), "claude/stop-tests-only.json");
    assert_eq!(common::stdout(&out), "");
    let lines = common::ledger_lines(dir.path(), "sess-1");
    let last = lines.last().unwrap();
    assert_eq!(last["kind"], "decision");
    assert_eq!(last["decision"], "allow");
    assert!(lines
        .iter()
        .any(|l| l["kind"] == "claim" && l["class"] == "tests-pass"));
}

#[test]
fn edit_after_the_run_invalidates_it_until_tests_run_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/app.py"), "print(1)\n").unwrap();
    hook(dir.path(), "claude/post-bash-pytest-pass.json");
    common::sleep_ms(5);
    hook(dir.path(), "claude/pre-edit.json");
    std::fs::write(dir.path().join("src/app.py"), "print(2)\n").unwrap();
    hook(dir.path(), "claude/post-edit.json");
    common::sleep_ms(5);
    let out = hook(dir.path(), "claude/stop-tests-only.json");
    let v: serde_json::Value = serde_json::from_str(common::stdout(&out).trim()).expect("block json");
    assert_eq!(v["decision"], "block");
    assert!(v["reason"].as_str().unwrap().contains("since the last edit"));
    common::sleep_ms(5);
    hook(dir.path(), "claude/post-bash-pytest-pass.json");
    let out = hook(dir.path(), "claude/stop-tests-only.json");
    assert_eq!(common::stdout(&out), "");
}

#[test]
fn failing_run_does_not_satisfy() {
    let dir = tempfile::tempdir().unwrap();
    hook(dir.path(), "claude/post-bash-pytest-fail.json");
    let out = hook(dir.path(), "claude/stop-tests-only.json");
    assert!(common::stdout(&out).contains("\"block\""));
}
