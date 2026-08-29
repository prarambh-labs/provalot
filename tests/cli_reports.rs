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
fn status_lists_enforced_and_advisory_rules() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        format!("{}/fixtures/policy/CLAUDE.md", env!("CARGO_MANIFEST_DIR")),
        dir.path().join("CLAUDE.md"),
    )
    .unwrap();
    let out = common::stdout(&common::run(&["status"], "", dir.path(), &[]));
    assert!(out.contains("deny-command"), "{out}");
    assert!(out.contains("require-before"));
    assert!(out.contains("protect-path"));
    assert!(out.contains("Advisory"));
    assert!(out.contains("MUST"));
}

#[test]
fn report_and_stats_describe_the_session() {
    let dir = tempfile::tempdir().unwrap();
    hook(dir.path(), "claude/post-bash-pytest-fail.json");
    assert!(hook(dir.path(), "claude/stop-tests-only.json").contains("\"block\""));
    let report_path = dir.path().join(".provalot/reports/sess-1.md");
    assert!(report_path.exists(), "report written at Stop");
    let out = common::stdout(&common::run(&["report", "sess-1"], "", dir.path(), &[]));
    assert!(out.contains("R1.tests-claimed-not-run"), "{out}");
    assert!(out.contains("pytest"));
    assert!(out.contains("fail"));
    let latest = common::stdout(&common::run(&["report"], "", dir.path(), &[]));
    assert_eq!(latest, out, "no argument means the latest session");
    let stats = common::stdout(&common::run(&["stats"], "", dir.path(), &[]));
    assert!(stats.contains("sessions: 1"), "{stats}");
    assert!(stats.contains("blocks: 1"));
    assert!(stats.contains("R1.tests-claimed-not-run: 1"));
}
