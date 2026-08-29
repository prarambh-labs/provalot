use crate::claims::{Claim, ClaimClass};
use crate::ledger::Line;

use super::Block;

pub const ID: &str = "R2.edit-claimed-no-change";

pub fn evaluate(lines: &[Line], claims: &[Claim]) -> Option<Block> {
    for c in claims.iter().filter(|c| c.class == ClaimClass::FileEdited) {
        let Some(path) = &c.path else { continue };
        let changed = lines
            .iter()
            .any(|l| matches!(l, Line::Edit { path: p, changed: true, .. } if p == path));
        if !changed {
            return Some(Block {
                rule: ID,
                reason: format!(
                    "[provalot] Claimed {path} was updated, but its content hash did not change in this session. Make the edit, or correct the message."
                ),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::{Claim, ClaimClass};

    fn edit(path: &str, changed: bool) -> Line {
        Line::Edit {
            ts: 1,
            agent_id: None,
            tool: "Edit".into(),
            path: path.into(),
            hash_before: None,
            hash_after: None,
            changed,
        }
    }
    fn claim(path: &str) -> Vec<Claim> {
        vec![Claim {
            class: ClaimClass::FileEdited,
            text: format!("updated {path}"),
            path: Some(path.into()),
        }]
    }

    #[test]
    fn claimed_path_without_a_changed_edit_blocks() {
        let b = evaluate(&[], &claim("src/app.py")).unwrap();
        assert_eq!(b.rule, ID);
        assert!(b.reason.contains("src/app.py"));
        assert!(evaluate(&[edit("src/app.py", false)], &claim("src/app.py")).is_some());
        assert!(evaluate(&[edit("src/other.py", true)], &claim("src/app.py")).is_some());
    }

    #[test]
    fn changed_edit_satisfies_and_no_claim_is_silent() {
        assert!(evaluate(&[edit("src/app.py", true)], &claim("src/app.py")).is_none());
        assert!(evaluate(&[], &[]).is_none());
    }
}
