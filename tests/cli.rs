mod common;

#[test]
fn version_prints_name_and_version() {
    let dir = tempfile::tempdir().unwrap();
    let out = common::run(&["--version"], "", dir.path(), &[]);
    assert!(out.status.success());
    assert!(
        common::stdout(&out).starts_with("provalot "),
        "got: {}",
        common::stdout(&out)
    );
}

#[test]
fn hook_with_unknown_harness_exits_zero_silently() {
    let dir = tempfile::tempdir().unwrap();
    let out = common::run(&["hook", "nope"], "{}", dir.path(), &[]);
    assert!(out.status.success());
    assert_eq!(common::stdout(&out), "");
}
