mod common;

fn hook(dir: &std::path::Path, fixture: &str) -> String {
    common::stdout(&common::run(
        &["hook", "codex"],
        &common::fixture(fixture, dir),
        dir,
        &[],
    ))
}

#[test]
fn codex_run_is_recorded_from_a_string_response() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(hook(dir.path(), "codex/post-bash-cargo-pass.json"), "");
    let lines = common::ledger_lines(dir.path(), "sess-cx");
    assert_eq!(lines[0]["runner"], "cargo");
    assert_eq!(lines[0]["outcome"], "pass");
}

#[test]
fn apply_patch_edit_is_recorded_per_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/app.py"), "print(1)\n").unwrap();
    hook(dir.path(), "codex/pre-apply-patch.json");
    std::fs::write(dir.path().join("src/app.py"), "print(2)\n").unwrap();
    hook(dir.path(), "codex/post-apply-patch.json");
    let lines = common::ledger_lines(dir.path(), "sess-cx");
    assert_eq!(lines[1]["kind"], "edit");
    assert_eq!(lines[1]["path"], "src/app.py");
    assert_eq!(lines[1]["changed"], true);
}

#[test]
fn codex_lying_stop_blocks_and_truthful_stop_allows() {
    let dir = tempfile::tempdir().unwrap();
    let out = hook(dir.path(), "codex/stop-lying.json");
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["decision"], "block");
    hook(dir.path(), "codex/post-bash-cargo-pass.json");
    assert_eq!(hook(dir.path(), "codex/stop-tests-only.json"), "");
}
