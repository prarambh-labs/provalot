mod common;

fn hook(dir: &std::path::Path, fixture: &str) -> String {
    common::stdout(&common::run(
        &["hook", "claude"],
        &common::fixture(fixture, dir),
        dir,
        &[],
    ))
}
fn decisions(dir: &std::path::Path) -> Vec<String> {
    common::ledger_lines(dir, "sess-lying")
        .iter()
        .filter(|l| l["kind"] == "decision")
        .map(|l| l["decision"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn retry_after_unverifiable_activity_is_softened_to_a_system_message() {
    let dir = tempfile::tempdir().unwrap();
    assert!(hook(dir.path(), "claude/stop-lying.json").contains("\"block\""));
    hook(dir.path(), "claude/post-bash-ls-lying.json");
    let out = hook(dir.path(), "claude/stop-lying-active.json");
    let v: serde_json::Value = serde_json::from_str(out.trim()).expect("json");
    assert!(v.get("decision").is_none(), "no block: {out}");
    assert!(v["systemMessage"].as_str().unwrap().starts_with("[provalot]"));
    assert_eq!(decisions(dir.path()), vec!["block", "softened"]);
}

#[test]
fn bare_retry_with_no_new_activity_is_blocked_again() {
    let dir = tempfile::tempdir().unwrap();
    assert!(hook(dir.path(), "claude/stop-lying.json").contains("\"block\""));
    assert!(hook(dir.path(), "claude/stop-lying-active.json").contains("\"block\""));
    assert_eq!(decisions(dir.path()), vec!["block", "block"]);
}

#[test]
fn retry_after_a_real_passing_run_is_a_plain_allow() {
    let dir = tempfile::tempdir().unwrap();
    assert!(hook(dir.path(), "claude/stop-lying.json").contains("\"block\""));
    hook(dir.path(), "claude/post-bash-pytest-pass-lying.json");
    assert_eq!(hook(dir.path(), "claude/stop-lying-active.json"), "");
    assert_eq!(decisions(dir.path()), vec!["block", "allow"]);
}
