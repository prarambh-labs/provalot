mod common;

#[test]
fn share_and_digest_run_offline_and_stay_de_identified() {
    let dir = tempfile::tempdir().unwrap();
    common::run(
        &["hook", "claude"],
        &common::fixture("claude/post-bash-pytest-pass.json", dir.path()),
        dir.path(),
        &[],
    );
    common::run(
        &["hook", "claude"],
        &common::fixture("claude/stop-lying.json", dir.path()),
        dir.path(),
        &[],
    );
    let share = common::stdout(&common::run(&["share"], "", dir.path(), &[]));
    assert!(share.contains("unbacked-claim rate"), "{share}");
    assert!(share.contains("Corpus median"), "{share}");
    assert!(share.contains("\"schema\": \"provalot-share/1\""), "{share}");
    assert!(!share.contains("pytest -q"), "commands must not leak: {share}");
    assert!(!share.contains(dir.path().to_str().unwrap()), "path leaked");
    let digest = common::stdout(&common::run(&["digest"], "", dir.path(), &[]));
    assert!(digest.contains("Near misses: 1"), "{digest}");
    assert!(!digest.contains("pytest -q"), "{digest}");
    let stats = common::stdout(&common::run(&["stats"], "", dir.path(), &[]));
    assert!(stats.contains("near-misses: 1"), "{stats}");
    let report = common::stdout(&common::run(&["report"], "", dir.path(), &[]));
    assert!(report.contains("## Near misses"), "{report}");
}
