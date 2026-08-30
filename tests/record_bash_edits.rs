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

#[test]
fn an_edit_and_a_test_run_in_one_command_record_the_edit_first() {
    let dir = setup();
    let pre = common::fixture("claude/pre-bash-sed.json", dir.path()).replace(
        "sed -i '' 's/print(1)/print(2)/' src/app.py &&",
        "sed -i '' 's/print(1)/print(2)/' src/app.py && pytest -q &&",
    );
    common::run(&["hook", "claude"], &pre, dir.path(), &[]);
    std::fs::write(dir.path().join("src/app.py"), "print(2)\n").unwrap();
    let post = common::fixture("claude/post-bash-sed.json", dir.path())
        .replace(
            "sed -i '' 's/print(1)/print(2)/' src/app.py &&",
            "sed -i '' 's/print(1)/print(2)/' src/app.py && pytest -q &&",
        )
        .replace("\"stdout\":\"\"", "\"stdout\":\"4 passed in 0.1s\\n\"");
    common::run(&["hook", "claude"], &post, dir.path(), &[]);
    let kinds: Vec<String> = common::ledger_lines(dir.path(), "sess-1")
        .iter()
        .map(|l| l["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        kinds,
        vec!["edit_pending", "edit_pending", "edit", "run"],
        "{kinds:?}"
    );
    assert_eq!(
        hook(dir.path(), "claude/stop-tests-only.json"),
        "",
        "the run in the same command counts"
    );
}
