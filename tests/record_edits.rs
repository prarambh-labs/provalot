mod common;

fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/app.py"), "print(1)\n").unwrap();
    dir
}

#[test]
fn edit_that_changes_the_file_is_recorded_as_changed() {
    let dir = setup();
    common::run(
        &["hook", "claude"],
        &common::fixture("claude/pre-edit.json", dir.path()),
        dir.path(),
        &[],
    );
    std::fs::write(dir.path().join("src/app.py"), "print(2)\n").unwrap();
    common::run(
        &["hook", "claude"],
        &common::fixture("claude/post-edit.json", dir.path()),
        dir.path(),
        &[],
    );
    let lines = common::ledger_lines(dir.path(), "sess-1");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["kind"], "edit_pending");
    assert_eq!(lines[0]["path"], "src/app.py");
    assert_eq!(lines[1]["kind"], "edit");
    assert_eq!(lines[1]["path"], "src/app.py");
    assert_eq!(lines[1]["changed"], true);
    assert_ne!(lines[1]["hash_before"], lines[1]["hash_after"]);
}

#[test]
fn edit_that_does_not_change_the_file_is_recorded_as_unchanged() {
    let dir = setup();
    common::run(
        &["hook", "claude"],
        &common::fixture("claude/pre-edit.json", dir.path()),
        dir.path(),
        &[],
    );
    common::run(
        &["hook", "claude"],
        &common::fixture("claude/post-edit.json", dir.path()),
        dir.path(),
        &[],
    );
    let lines = common::ledger_lines(dir.path(), "sess-1");
    assert_eq!(lines[1]["changed"], false);
}

#[test]
fn write_of_a_new_file_is_changed() {
    let dir = setup();
    common::run(
        &["hook", "claude"],
        &common::fixture("claude/pre-write.json", dir.path()),
        dir.path(),
        &[],
    );
    std::fs::create_dir_all(dir.path().join("migrations")).unwrap();
    std::fs::write(
        dir.path().join("migrations/0002_add.sql"),
        "ALTER TABLE x ADD y INT;",
    )
    .unwrap();
    common::run(
        &["hook", "claude"],
        &common::fixture("claude/post-write.json", dir.path()),
        dir.path(),
        &[],
    );
    let lines = common::ledger_lines(dir.path(), "sess-1");
    assert_eq!(lines[1]["kind"], "edit");
    assert!(lines[1]["hash_before"].is_null());
    assert_eq!(lines[1]["changed"], true);
}
