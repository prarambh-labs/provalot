mod common;

fn hook(dir: &std::path::Path, fixture: &str) -> std::process::Output {
    common::run(&["hook", "claude"], &common::fixture(fixture, dir), dir, &[])
}

#[test]
fn claimed_edit_without_change_blocks_then_real_edit_allows() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/app.py"), "print(1)\n").unwrap();
    let out = hook(dir.path(), "claude/stop-edit-claim.json");
    let v: serde_json::Value = serde_json::from_str(common::stdout(&out).trim()).expect("block json");
    assert_eq!(v["decision"], "block");
    assert!(v["reason"].as_str().unwrap().contains("src/app.py"));
    hook(dir.path(), "claude/pre-edit.json");
    std::fs::write(dir.path().join("src/app.py"), "print(2)\n").unwrap();
    hook(dir.path(), "claude/post-edit.json");
    let out = hook(dir.path(), "claude/stop-edit-claim.json");
    assert_eq!(common::stdout(&out), "");
}

#[test]
fn claim_about_a_file_that_does_not_exist_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let out = hook(dir.path(), "claude/stop-edit-claim.json");
    assert_eq!(common::stdout(&out), "");
}
