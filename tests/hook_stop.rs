mod common;

#[test]
fn lying_stop_is_blocked_with_reason_for_the_model() {
    let dir = tempfile::tempdir().unwrap();
    let payload = common::fixture("claude/stop-lying.json", dir.path());
    let out = common::run(&["hook", "claude"], &payload, dir.path(), &[]);
    assert!(
        out.status.success(),
        "hook must exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_str(common::stdout(&out).trim()).expect("json on stdout");
    assert_eq!(v["decision"], "block");
    let reason = v["reason"].as_str().unwrap();
    assert!(reason.starts_with("[provalot]"));
    assert!(reason.contains("tests"));
}
