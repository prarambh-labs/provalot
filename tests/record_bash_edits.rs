mod common;

fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join("notes")).unwrap();
    std::fs::write(dir.path().join("src/app.py"), "print(1)\n").unwrap();
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

#[test]
fn bash_edit_that_changes_a_named_file_is_recorded_and_backs_the_claim() {
    let dir = setup();
    hook(dir.path(), "claude/pre-bash-sed.json");
    std::fs::write(dir.path().join("src/app.py"), "print(2)\n").unwrap();
    std::fs::write(dir.path().join("notes/new.md"), "hello\n").unwrap();
    hook(dir.path(), "claude/post-bash-sed.json");
    let lines = common::ledger_lines(dir.path(), "sess-1");
    let edits: Vec<&serde_json::Value> = lines.iter().filter(|l| l["kind"] == "edit").collect();
    assert_eq!(edits.len(), 2, "{lines:?}");
    assert!(edits.iter().all(|e| e["tool"] == "Bash" && e["changed"] == true));
    assert!(edits
        .iter()
        .any(|e| e["path"] == "src/app.py" && !e["hash_before"].is_null()));
    assert!(edits
        .iter()
        .any(|e| e["path"] == "notes/new.md" && e["hash_before"].is_null()));
    assert_eq!(
        hook(dir.path(), "claude/stop-edit-claim.json"),
        "",
        "truthful claim after a Bash edit is allowed"
    );
}

#[test]
fn bash_that_only_mentions_a_file_records_no_edit() {
    let dir = setup();
    hook(dir.path(), "claude/pre-bash-sed.json");
    hook(dir.path(), "claude/post-bash-sed.json");
    let lines = common::ledger_lines(dir.path(), "sess-1");
    assert!(lines.iter().all(|l| l["kind"] != "edit"), "{lines:?}");
    assert!(
        hook(dir.path(), "claude/stop-edit-claim.json").contains("\"block\""),
        "unchanged file still blocks"
    );
}
