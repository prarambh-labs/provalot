use crate::claims::{Claim, ClaimClass};
use crate::evidence::runner::Runner;
use crate::ledger::Line;

use super::Block;

pub const ID: &str = "R1.tests-claimed-not-run";

pub fn last_changed_edit(lines: &[Line]) -> Option<(u64, String)> {
    lines.iter().rev().find_map(|l| match l {
        Line::Edit {
            ts,
            path,
            changed: true,
            ..
        } => Some((*ts, path.clone())),
        _ => None,
    })
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
    Some(Block {
        rule: ID,
        reason: format!(
            "[provalot] Claimed tests pass, but no test runner has passed since the last edit ({edit}; {run}). Run the tests now, or say they were not run."
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
}
