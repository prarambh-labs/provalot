mod common;

#[test]
fn malformed_stdin_exits_zero_with_no_stdout_and_logs() {
    let dir = tempfile::tempdir().unwrap();
    let out = common::run(&["hook", "claude"], "this is not json", dir.path(), &[]);
    assert!(out.status.success());
    assert_eq!(common::stdout(&out), "");
    let log = std::fs::read_to_string(dir.path().join(".provalot/errors.log")).expect("errors.log written");
    assert!(log.contains("bad json"), "log was: {log}");
}

#[test]
fn missing_fields_exit_zero_with_no_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let out = common::run(
        &["hook", "claude"],
        r#"{"hook_event_name":"Stop"}"#,
        dir.path(),
        &[],
    );
    assert!(out.status.success());
    assert_eq!(common::stdout(&out), "");
}
