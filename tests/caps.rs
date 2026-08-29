mod common;

fn lying(dir: &std::path::Path) -> String {
    common::stdout(&common::run(
        &["hook", "claude"],
        &common::fixture("claude/stop-lying.json", dir),
        dir,
        &[],
    ))
}

#[test]
fn fourth_consecutive_block_is_capped_to_allow() {
    let dir = tempfile::tempdir().unwrap();
    for n in 1..=3 {
        assert!(lying(dir.path()).contains("\"block\""), "block #{n}");
    }
    assert_eq!(lying(dir.path()), "", "capped");
    let lines = common::ledger_lines(dir.path(), "sess-lying");
    let decisions: Vec<String> = lines
        .iter()
        .filter(|l| l["kind"] == "decision")
        .map(|l| l["decision"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(decisions, vec!["block", "block", "block", "capped"]);
    assert_eq!(
        lines.iter().filter(|l| l["kind"] == "decision").nth(2).unwrap()["consecutive"],
        3
    );
}

#[test]
fn allow_once_overrides_the_next_stop_and_is_logged() {
    let dir = tempfile::tempdir().unwrap();
    assert!(lying(dir.path()).contains("\"block\""));
    let out = common::run(&["allow", "--once"], "", dir.path(), &[]);
    assert!(out.status.success());
    assert!(common::stdout(&out).contains("sess-lying"));
    assert_eq!(lying(dir.path()), "");
    let lines = common::ledger_lines(dir.path(), "sess-lying");
    assert_eq!(lines.last().unwrap()["decision"], "override");
    assert!(lying(dir.path()).contains("\"block\""), "override is single use");
}
