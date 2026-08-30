use std::collections::BTreeMap;
use std::path::Path;

use crate::ledger::{self, Line};

pub fn render(root: &Path) -> String {
    let sessions = ledger::all_sessions(root);
    let (mut claims, mut blocks, mut allows, mut capped, mut overrides, mut softened) = (0, 0, 0, 0, 0, 0);
    let mut by_rule: BTreeMap<String, u32> = BTreeMap::new();
    for s in &sessions {
        for l in ledger::read(root, s) {
            match l {
                Line::Claim { .. } => claims += 1,
                Line::Decision { decision, rule, .. } => match decision.as_str() {
                    "block" => {
                        blocks += 1;
                        *by_rule.entry(rule).or_default() += 1;
                    }
                    "allow" => allows += 1,
                    "capped" => capped += 1,
                    "override" => overrides += 1,
                    "softened" => softened += 1,
                    _ => {}
                },
                _ => {}
            }
        }
    }
    let mut out = format!(
        "sessions: {}\nclaims: {claims}\nblocks: {blocks}\nallows: {allows}\ncapped: {capped}\noverrides: {overrides}\nsoftened: {softened}\n",
        sessions.len()
    );
    out.push_str("blocks by rule:\n");
    for (r, n) in by_rule {
        out.push_str(&format!("  {r}: {n}\n"));
    }
    out
}
