use std::path::Path;

use crate::ledger::{self, Line};
use crate::rules::Block;

pub const MAX_CONSECUTIVE: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Block {
        rule: &'static str,
        reason: String,
        consecutive: u32,
    },
    Capped {
        rule: &'static str,
    },
    Overridden,
    /// `stop_hook_active` retry after new runs/edits that still leave the claim unverified.
    Softened {
        rule: &'static str,
        reason: String,
    },
}

/// Trailing run of `block` decisions for `rule`; any other decision ends the run.
pub fn consecutive_blocks(lines: &[Line], rule: &str) -> u32 {
    let mut n = 0;
    for l in lines.iter().rev() {
        match l {
            Line::Decision {
                decision, rule: r, ..
            } if decision == "block" && r == rule => n += 1,
            Line::Decision { .. } => break,
            _ => {}
        }
    }
    n
}

/// An `override` line with no decision after it.
pub fn has_unconsumed_override(lines: &[Line]) -> bool {
    for l in lines.iter().rev() {
        match l {
            Line::Override { .. } => return true,
            Line::Decision { .. } => return false,
            _ => {}
        }
    }
    false
}

/// True when the most recent decision was a block for `rule` and the agent has recorded a run or
/// edit since — it retried and did something, even if the ledger still cannot verify the claim.
pub fn retried_with_activity(lines: &[Line], rule: &str) -> bool {
    let mut activity = false;
    for l in lines.iter().rev() {
        match l {
            Line::Run { .. } | Line::Edit { .. } => activity = true,
            Line::Decision {
                decision, rule: r, ..
            } => return decision == "block" && r == rule && activity,
            _ => {}
        }
    }
    false
}

/// `retry` is the harness's `stop_hook_active`: a Stop re-evaluated after an earlier block this
/// turn. A retry that shows new activity is softened to a warning instead of a second block; a
/// bare re-claim with nothing new in the ledger is blocked again (up to the cap).
pub fn verdict(lines: &[Line], blocks: Vec<Block>, cap: bool, retry: bool) -> Verdict {
    let Some(b) = blocks.into_iter().next() else {
        return Verdict::Allow;
    };
    if has_unconsumed_override(lines) {
        return Verdict::Overridden;
    }
    if retry && retried_with_activity(lines, b.rule) {
        return Verdict::Softened {
            rule: b.rule,
            reason: format!(
                "[provalot] Retried with new activity, but the ledger still cannot verify the claim. Allowing this once; check `provalot report`. Original reason: {}",
                b.reason.trim_start_matches("[provalot] ")
            ),
        };
    }
    let n = consecutive_blocks(lines, b.rule);
    if cap && n >= MAX_CONSECUTIVE {
        return Verdict::Capped { rule: b.rule };
    }
    Verdict::Block {
        rule: b.rule,
        reason: b.reason,
        consecutive: n + 1,
    }
}

/// `provalot allow --once`: permit the next blocking decision in the latest session.
pub fn record_override(root: &Path) -> Result<String, String> {
    let session = ledger::latest_session(root).ok_or("no session ledger found under .provalot/sessions")?;
    ledger::append(root, &session, &Line::Override { ts: ledger::now_ms() })?;
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Block;

    fn dec(decision: &str, rule: &str) -> Line {
        Line::Decision {
            ts: 1,
            agent_id: None,
            event: "Stop".into(),
            decision: decision.into(),
            rule: rule.into(),
            reason: String::new(),
            consecutive: 0,
        }
    }
    fn block() -> Vec<Block> {
        vec![Block {
            rule: "R1.tests-claimed-not-run",
            reason: "r".into(),
        }]
    }

    #[test]
    fn counts_trailing_blocks_for_the_same_rule() {
        let lines = [
            dec("block", "R1.tests-claimed-not-run"),
            dec("block", "R1.tests-claimed-not-run"),
            Line::Override { ts: 2 },
        ];
        assert_eq!(consecutive_blocks(&lines, "R1.tests-claimed-not-run"), 2);
        let lines = [dec("block", "R1.tests-claimed-not-run"), dec("allow", "")];
        assert_eq!(consecutive_blocks(&lines, "R1.tests-claimed-not-run"), 0);
        let lines = [dec("block", "R2.edit-claimed-no-change")];
        assert_eq!(consecutive_blocks(&lines, "R1.tests-claimed-not-run"), 0);
    }

    #[test]
    fn retry_with_activity_is_softened_but_a_bare_retry_is_not() {
        let run = Line::Run {
            ts: 2,
            agent_id: None,
            tool: "Bash".into(),
            command: "ls".into(),
            runner: "other".into(),
            outcome: "unknown".into(),
            stdout_hash: String::new(),
            stderr_hash: String::new(),
            is_error: false,
            interrupted: false,
        };
        let blocked = vec![dec("block", "R1.tests-claimed-not-run")];
        assert!(
            matches!(
                verdict(&blocked, block(), true, true),
                Verdict::Block { consecutive: 2, .. }
            ),
            "bare retry blocks"
        );
        let active = vec![dec("block", "R1.tests-claimed-not-run"), run.clone()];
        assert!(matches!(
            verdict(&active, block(), true, true),
            Verdict::Softened { .. }
        ));
        assert!(
            matches!(verdict(&active, block(), true, false), Verdict::Block { .. }),
            "not a retry"
        );
        let other_rule = vec![dec("block", "R2.edit-claimed-no-change"), run];
        assert!(matches!(
            verdict(&other_rule, block(), true, true),
            Verdict::Block { consecutive: 1, .. }
        ));
    }

    #[test]
    fn caps_after_three_and_honours_override() {
        let three = vec![dec("block", "R1.tests-claimed-not-run"); 3];
        assert!(matches!(
            verdict(&three, block(), true, false),
            Verdict::Capped { .. }
        ));
        assert!(
            matches!(
                verdict(&three, block(), false, false),
                Verdict::Block { consecutive: 4, .. }
            ),
            "no cap for PreToolUse denies"
        );
        let two = vec![dec("block", "R1.tests-claimed-not-run"); 2];
        assert!(matches!(
            verdict(&two, block(), true, false),
            Verdict::Block { consecutive: 3, .. }
        ));
        let overridden = vec![dec("block", "R1.tests-claimed-not-run"), Line::Override { ts: 9 }];
        assert!(matches!(
            verdict(&overridden, block(), true, false),
            Verdict::Overridden
        ));
        let consumed = vec![
            Line::Override { ts: 9 },
            dec("override", "R1.tests-claimed-not-run"),
        ];
        assert!(matches!(
            verdict(&consumed, block(), true, false),
            Verdict::Block { consecutive: 1, .. }
        ));
        assert!(matches!(verdict(&[], vec![], true, false), Verdict::Allow));
    }
}
