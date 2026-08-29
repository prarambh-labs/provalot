mod common;

#[test]
fn post_bash_pytest_pass_is_recorded_as_a_passing_run() {
    let dir = tempfile::tempdir().unwrap();
    let payload = common::fixture("claude/post-bash-pytest-pass.json", dir.path());
    let out = common::run(&["hook", "claude"], &payload, dir.path(), &[]);
    assert!(out.status.success());
    assert_eq!(common::stdout(&out), "", "PostToolUse never prints");
    let lines = common::ledger_lines(dir.path(), "sess-1");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["kind"], "run");
    assert_eq!(lines[0]["runner"], "pytest");
    assert_eq!(lines[0]["outcome"], "pass");
    assert_eq!(lines[0]["command"], "pytest -q");
    assert!(lines[0]["stdout_hash"].as_str().unwrap().len() == 64);
    assert!(lines[0].get("stdout").is_none(), "never store output text");
}

#[test]
fn post_bash_pytest_fail_is_recorded_as_failing() {
    let dir = tempfile::tempdir().unwrap();
    let payload = common::fixture("claude/post-bash-pytest-fail.json", dir.path());
    common::run(&["hook", "claude"], &payload, dir.path(), &[]);
    let lines = common::ledger_lines(dir.path(), "sess-1");
    assert_eq!(lines[0]["outcome"], "fail");
    assert_eq!(lines[0]["is_error"], true);
}
