mod common;

#[test]
fn hook_path_stays_fast_with_a_large_ledger() {
    let dir = tempfile::tempdir().unwrap();
    for _ in 0..200 {
        common::run(
            &["hook", "claude"],
            &common::fixture("claude/post-bash-pytest-pass.json", dir.path()),
            dir.path(),
            &[],
        );
    }
    let payload = common::fixture("claude/stop-tests-only.json", dir.path());
    let mut worst = std::time::Duration::ZERO;
    for _ in 0..20 {
        let t = std::time::Instant::now();
        common::run(&["hook", "claude"], &payload, dir.path(), &[]);
        worst = worst.max(t.elapsed());
    }
    // Debug build plus process spawn; release p95 is measured by scripts/bench.sh against 50 ms.
    assert!(
        worst < std::time::Duration::from_millis(500),
        "worst hook took {worst:?}"
    );
}
