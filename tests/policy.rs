mod common;

fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let src = format!("{}/fixtures/policy/CLAUDE.md", env!("CARGO_MANIFEST_DIR"));
    std::fs::copy(src, dir.path().join("CLAUDE.md")).unwrap();
    dir
}
fn hook(dir: &std::path::Path, fixture: &str) -> String {
    common::stdout(&common::run(
        &["hook", "claude"],
        &common::fixture(fixture, dir),
        dir,
        &[],
    ))
}
fn is_deny(s: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(s.trim())
        .map(|v| v["hookSpecificOutput"]["permissionDecision"] == "deny")
        .unwrap_or(false)
}

#[test]
fn force_push_is_denied_and_ls_is_not() {
    let dir = setup();
    let out = hook(dir.path(), "claude/pre-bash-force-push.json");
    assert!(is_deny(&out), "{out}");
    assert!(out.contains("NEVER run"));
    assert_eq!(hook(dir.path(), "claude/pre-bash-ls.json"), "");
    let lines = common::ledger_lines(dir.path(), "sess-1");
    assert!(lines
        .iter()
        .any(|l| l["kind"] == "decision" && l["rule"] == "R3.deny-command"));
}

#[test]
fn commit_needs_a_passing_run_first() {
    let dir = setup();
    assert!(is_deny(&hook(dir.path(), "claude/pre-bash-commit.json")));
    hook(dir.path(), "claude/post-bash-pytest-pass.json");
    assert_eq!(hook(dir.path(), "claude/pre-bash-commit.json"), "");
}

#[test]
fn protected_path_is_denied_until_allowed_once() {
    let dir = setup();
    assert!(is_deny(&hook(dir.path(), "claude/pre-write.json")));
    common::run(&["allow", "--once"], "", dir.path(), &[]);
    assert_eq!(hook(dir.path(), "claude/pre-write.json"), "");
    assert!(is_deny(&hook(dir.path(), "claude/pre-write.json")), "single use");
}

#[test]
fn without_a_policy_file_nothing_is_denied() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(hook(dir.path(), "claude/pre-bash-force-push.json"), "");
}
