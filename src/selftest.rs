use std::path::{Path, PathBuf};

use crate::event::Harness;
use crate::hook;

const STOP_LYING: &str = include_str!("../fixtures/hooks/claude/stop-lying.json");
const STOP_TESTS_ONLY: &str = include_str!("../fixtures/hooks/claude/stop-tests-only.json");
const STOP_EDIT_CLAIM: &str = include_str!("../fixtures/hooks/claude/stop-edit-claim.json");
const POST_PYTEST_PASS: &str = include_str!("../fixtures/hooks/claude/post-bash-pytest-pass.json");
const PRE_EDIT: &str = include_str!("../fixtures/hooks/claude/pre-edit.json");
const POST_EDIT: &str = include_str!("../fixtures/hooks/claude/post-edit.json");
const PRE_FORCE_PUSH: &str = include_str!("../fixtures/hooks/claude/pre-bash-force-push.json");
const PRE_COMMIT: &str = include_str!("../fixtures/hooks/claude/pre-bash-commit.json");
const PRE_BASH_SED: &str = include_str!("../fixtures/hooks/claude/pre-bash-sed.json");
const POST_BASH_SED: &str = include_str!("../fixtures/hooks/claude/post-bash-sed.json");
const PRE_WRITE: &str = include_str!("../fixtures/hooks/claude/pre-write.json");
const POLICY: &str = include_str!("../fixtures/policy/CLAUDE.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    Allow,
    Block,
    Deny,
}

fn fx(template: &str, cwd: &Path) -> String {
    let cwd_json = serde_json::to_string(cwd.to_str().unwrap_or(".")).unwrap();
    let bare = cwd_json.trim_matches('"').to_string();
    template
        .replace("\"__CWD__\"", &cwd_json)
        .replace("__CWD__", &bare)
}

fn fire(cwd: &Path, template: &str) -> Result<String, String> {
    hook::run(Harness::Claude, &fx(template, cwd)).map(|o| o.stdout.unwrap_or_default())
}

fn observed(stdout: &str) -> Expect {
    match serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        Ok(v) if v["decision"] == "block" => Expect::Block,
        Ok(v) if v["hookSpecificOutput"]["permissionDecision"] == "deny" => Expect::Deny,
        _ => Expect::Allow,
    }
}

fn case_dir(base: &Path, n: usize) -> PathBuf {
    let d = base.join(format!("case{n}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("src")).expect("selftest dir");
    std::fs::write(d.join("src/app.py"), "print(1)\n").expect("seed file");
    d
}

fn check(name: &str, results: &mut Vec<(String, bool, String)>, got: Result<String, String>, want: Expect) {
    match got {
        Ok(out) => {
            let o = observed(&out);
            results.push((name.into(), o == want, format!("expected {want:?}, got {o:?}")));
        }
        Err(e) => results.push((name.into(), false, format!("hook error: {e}"))),
    }
}

pub fn run() -> Vec<(String, bool, String)> {
    let base = std::env::temp_dir().join(format!("provalot-selftest-{}", std::process::id()));
    let mut r = Vec::new();

    let d = case_dir(&base, 1);
    check(
        "R1 blocks a Stop that claims tests pass with an empty ledger",
        &mut r,
        fire(&d, STOP_LYING),
        Expect::Block,
    );

    let d = case_dir(&base, 2);
    let _ = fire(&d, POST_PYTEST_PASS);
    check(
        "R1 allows the claim after a passing run",
        &mut r,
        fire(&d, STOP_TESTS_ONLY),
        Expect::Allow,
    );

    let d = case_dir(&base, 3);
    check(
        "R2 blocks an edit claim when the file did not change",
        &mut r,
        fire(&d, STOP_EDIT_CLAIM),
        Expect::Block,
    );

    let d = case_dir(&base, 4);
    let _ = fire(&d, PRE_EDIT);
    std::fs::write(d.join("src/app.py"), "print(2)\n").expect("edit");
    let _ = fire(&d, POST_EDIT);
    check(
        "R2 allows the edit claim after a real change",
        &mut r,
        fire(&d, STOP_EDIT_CLAIM),
        Expect::Allow,
    );

    let d = case_dir(&base, 5);
    std::fs::write(d.join("CLAUDE.md"), POLICY).expect("policy");
    check(
        "R3 denies a NEVER-run command",
        &mut r,
        fire(&d, PRE_FORCE_PUSH),
        Expect::Deny,
    );

    let d = case_dir(&base, 6);
    std::fs::write(d.join("CLAUDE.md"), POLICY).expect("policy");
    let first = fire(&d, PRE_COMMIT);
    let _ = fire(&d, POST_PYTEST_PASS);
    let second = fire(&d, PRE_COMMIT);
    let ok = first
        .as_ref()
        .map(|s| observed(s) == Expect::Deny)
        .unwrap_or(false)
        && second
            .as_ref()
            .map(|s| observed(s) == Expect::Allow)
            .unwrap_or(false);
    r.push((
        "R3 requires a passing test run before git commit".into(),
        ok,
        format!("first {first:?}, second {second:?}"),
    ));

    let d = case_dir(&base, 7);
    std::fs::write(d.join("CLAUDE.md"), POLICY).expect("policy");
    check(
        "R3 denies an edit under a protected path",
        &mut r,
        fire(&d, PRE_WRITE),
        Expect::Deny,
    );

    let d = case_dir(&base, 8);
    std::fs::create_dir_all(d.join("notes")).expect("notes dir");
    let _ = fire(&d, PRE_BASH_SED);
    std::fs::write(d.join("src/app.py"), "print(2)\n").expect("edit");
    let _ = fire(&d, POST_BASH_SED);
    check(
        "R2 allows the edit claim after a change made through Bash",
        &mut r,
        fire(&d, STOP_EDIT_CLAIM),
        Expect::Allow,
    );

    let _ = std::fs::remove_dir_all(&base);
    r
}
