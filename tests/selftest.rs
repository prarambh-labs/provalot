mod common;

#[test]
fn selftest_passes_and_reports_seven_cases() {
    let dir = tempfile::tempdir().unwrap();
    let out = common::run(&["selftest"], "", dir.path(), &[]);
    let text = common::stdout(&out);
    assert!(out.status.success(), "{text}");
    assert_eq!(text.matches("PASS ").count(), 7, "{text}");
}

#[test]
fn selftest_fails_when_a_rule_is_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let out = common::run(&["selftest"], "", dir.path(), &[("PROVALOT_DISABLE_RULE", "R1")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(common::stdout(&out).contains("FAIL R1"));
}
