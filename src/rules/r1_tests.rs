use crate::claims::{Claim, ClaimClass};
use crate::evidence::runner::Runner;
use crate::ledger::Line;

use super::Block;

pub const ID: &str = "R1.tests-claimed-not-run";

/// Paths whose edits cannot change a test outcome: prose and documentation. An edit to
/// NEXT_STEP.md after a green run does not make "tests pass" an unbacked claim. First-party
/// audit (2026-09-04): 54 of 96 blocks were exactly this shape.
pub fn affects_tests(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    let doc_ext = [".md", ".markdown", ".txt", ".rst", ".adoc", ".org"];
    if doc_ext.iter().any(|e| name.ends_with(e)) {
        return false;
    }
    if matches!(name, "license" | "licence" | "changelog" | "authors" | "notice") {
        return false;
    }
    let in_docs = lower.starts_with("docs/") || lower.contains("/docs/");
    !in_docs
}

pub fn last_changed_edit(lines: &[Line]) -> Option<(u64, String)> {
    lines.iter().rev().find_map(|l| match l {
        Line::Edit {
            ts,
            path,
            changed: true,
            ..
        } if affects_tests(path) => Some((*ts, path.clone())),
        _ => None,
    })
}

/// The last run by a recognised test runner, whatever its outcome.
pub fn last_runner_run(lines: &[Line]) -> Option<(u64, String, String)> {
    lines.iter().rev().find_map(|l| match l {
        Line::Run {
            ts,
            command,
            runner,
            outcome,
            ..
        } if runner != "other" => Some((*ts, command.clone(), outcome.clone())),
        _ => None,
    })
}

fn is_piped(command: &str) -> bool {
    let mut quote: Option<char> = None;
    for c in command.chars() {
        match c {
            '\'' | '"' => match quote {
                Some(q) if q == c => quote = None,
                None => quote = Some(c),
                _ => {}
            },
            '|' if quote.is_none() => return true,
            _ => {}
        }
    }
    false
}

pub fn last_passing_run(lines: &[Line], want: Option<Runner>) -> Option<(u64, String)> {
    lines.iter().rev().find_map(|l| match l {
        Line::Run {
            ts,
            command,
            runner,
            outcome,
            ..
        } if outcome == "pass"
            && runner != "other"
            && want.map(|w| Runner::parse(runner) == w).unwrap_or(true) =>
        {
            Some((*ts, command.clone()))
        }
        _ => None,
    })
}

/// True when a passing test run exists after the last changed edit (or with no edits at all).
pub fn evidence_current(lines: &[Line], want: Option<Runner>) -> bool {
    match (last_changed_edit(lines), last_passing_run(lines, want)) {
        (None, Some(_)) => true,
        (Some((et, _)), Some((rt, _))) => rt >= et,
        _ => false,
    }
}

/// First line of a command, cut to 80 chars, so a heredoc never lands in a block reason.
fn summary(command: &str) -> String {
    let first = command.lines().next().unwrap_or("");
    if first.chars().count() > 80 {
        format!("{}…", first.chars().take(80).collect::<String>())
    } else if command.contains('\n') {
        format!("{first}…")
    } else {
        first.to_string()
    }
}

pub fn evaluate(lines: &[Line], claims: &[Claim]) -> Option<Block> {
    if !claims.iter().any(|c| c.class == ClaimClass::TestsPass) {
        return None;
    }
    if evidence_current(lines, None) {
        return None;
    }
    let edit = last_changed_edit(lines)
        .map(|(_, p)| format!("last edit: {p}"))
        .unwrap_or_else(|| "no edits recorded".into());
    let run = last_passing_run(lines, None)
        .map(|(_, c)| format!("last passing run: {}", summary(&c)))
        .unwrap_or_else(|| "no passing test run recorded".into());
    let unread = match last_runner_run(lines) {
        Some((_, c, o)) if o == "unknown" => {
            if is_piped(&c) {
                format!(
                    " The result of `{}` could not be read because its output was filtered through a pipe; rerun it without grep/tail/head.",
                    summary(&c)
                )
            } else {
                format!(
                    " The result of `{}` could not be read from its output.",
                    summary(&c)
                )
            }
        }
        _ => String::new(),
    };
    Some(Block {
        rule: ID,
        reason: format!(
            "[provalot] Claimed tests pass, but no test runner has passed since the last edit ({edit}; {run}).{unread} Run the tests now, or say they were not run."
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::{Claim, ClaimClass};

    fn run(ts: u64, runner: &str, outcome: &str) -> Line {
        Line::Run {
            ts,
            agent_id: None,
            tool: "Bash".into(),
            command: format!("{runner} cmd"),
            runner: runner.into(),
            outcome: outcome.into(),
            stdout_hash: "".into(),
            stderr_hash: "".into(),
            is_error: false,
            interrupted: false,
        }
    }
    fn edit_at(ts: u64, changed: bool, path: &str) -> Line {
        Line::Edit {
            ts,
            agent_id: None,
            tool: "Edit".into(),
            path: path.into(),
            hash_before: None,
            hash_after: None,
            changed,
        }
    }
    fn edit(ts: u64, changed: bool) -> Line {
        Line::Edit {
            ts,
            agent_id: None,
            tool: "Edit".into(),
            path: "src/app.py".into(),
            hash_before: None,
            hash_after: None,
            changed,
        }
    }
    fn claim() -> Vec<Claim> {
        vec![Claim {
            class: ClaimClass::TestsPass,
            text: "All tests pass".into(),
            path: None,
        }]
    }

    #[test]
    fn no_claim_means_no_block() {
        assert!(evaluate(&[], &[]).is_none());
    }

    #[test]
    fn claim_with_empty_ledger_blocks() {
        let b = evaluate(&[], &claim()).unwrap();
        assert_eq!(b.rule, ID);
        assert!(b.reason.contains("no passing test run"));
    }

    #[test]
    fn passing_run_after_last_edit_satisfies() {
        assert!(evaluate(&[edit(10, true), run(20, "pytest", "pass")], &claim()).is_none());
        assert!(evaluate(&[run(20, "pytest", "pass")], &claim()).is_none());
    }

    #[test]
    fn run_before_last_edit_or_failing_or_other_does_not_satisfy() {
        assert!(evaluate(&[run(10, "pytest", "pass"), edit(20, true)], &claim()).is_some());
        assert!(evaluate(&[run(20, "pytest", "fail")], &claim()).is_some());
        assert!(evaluate(&[run(20, "pytest", "unknown")], &claim()).is_some());
        assert!(evaluate(&[run(20, "other", "pass")], &claim()).is_some());
        assert!(
            evaluate(&[run(10, "pytest", "pass"), edit(20, false)], &claim()).is_none(),
            "unchanged edits do not invalidate"
        );
    }

    #[test]
    fn doc_edits_after_a_green_run_do_not_invalidate_it() {
        for p in [
            "NEXT_STEP.md",
            "docs/plan.md",
            "README.txt",
            "notes/x.rst",
            "LICENSE",
            "a/docs/b.py",
        ] {
            assert!(
                evaluate(&[run(10, "pytest", "pass"), edit_at(20, true, p)], &claim()).is_none(),
                "{p} should not invalidate"
            );
        }
        assert!(evaluate(
            &[run(10, "pytest", "pass"), edit_at(20, true, "src/lib.rs")],
            &claim()
        )
        .is_some());
        assert!(evaluate(
            &[run(10, "pytest", "pass"), edit_at(20, true, "tests/test_x.py")],
            &claim()
        )
        .is_some());
    }

    #[test]
    fn unreadable_outcome_is_named_in_the_reason() {
        let mut piped = run(20, "cargo", "unknown");
        if let Line::Run { command, .. } = &mut piped {
            *command = "cargo test 2>&1 | grep -E \"^error\"".into();
        }
        let b = evaluate(&[piped], &claim()).unwrap();
        assert!(b.reason.contains("filtered through a pipe"), "{}", b.reason);
        let plain = run(20, "pytest", "unknown");
        let b = evaluate(&[plain], &claim()).unwrap();
        assert!(
            b.reason.contains("could not be read from its output"),
            "{}",
            b.reason
        );
        assert!(!b.reason.contains("pipe"));
    }

    #[test]
    fn pipe_inside_quotes_is_not_a_pipe() {
        assert!(!is_piped("grep -n 'a\\|b' f.md"));
        assert!(is_piped("cargo test 2>&1 | tail -5"));
    }
}
