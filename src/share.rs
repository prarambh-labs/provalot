//! ASRS-style voluntary sharing: a de-identified stats blob plus an on-the-spot benchmark.
//!
//! De-identification is architectural, not a policy promise: `FleetStats` holds only counters
//! keyed by provalot's own fixed vocabulary (rule ids, runner names, claim classes, decision
//! kinds). No field ever carries a command, a path, or message text, so the serialized blob
//! cannot leak them — `nothing_from_the_ledger_text_leaks` proves it against a seeded ledger.
//! Nothing is transmitted anywhere: `share` prints the blob and the user decides.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::ledger::{self, Line};
use crate::repo;

/// Embedded corpus baseline, updated per release from voluntarily shared blobs.
const BASELINE: &str = include_str!("../fixtures/corpus-baseline.json");

#[derive(Debug, Default, Serialize)]
pub struct FleetStats {
    pub sessions: u32,
    /// Stop/SubagentStop evaluations (every decision at a stop, whatever its outcome).
    pub stops: u32,
    /// Blocks at a stop: an unbacked claim that would have shipped.
    pub near_misses: u32,
    pub allows: u32,
    pub softened: u32,
    pub capped: u32,
    pub overrides: u32,
    /// PreToolUse policy denies.
    pub denies: u32,
    pub blocks_by_rule: BTreeMap<String, u32>,
    pub claims_by_class: BTreeMap<String, u32>,
    pub runs_by_runner: BTreeMap<String, u32>,
    pub runs_pass: u32,
    pub runs_fail: u32,
    pub runs_unknown: u32,
    pub first_ts: u64,
    pub last_ts: u64,
}

pub fn collect(root: &Path) -> FleetStats {
    let mut f = FleetStats::default();
    for session in ledger::all_sessions(root) {
        f.sessions += 1;
        for l in ledger::read(root, &session) {
            match l {
                Line::Decision {
                    ts,
                    event,
                    decision,
                    rule,
                    ..
                } => {
                    f.first_ts = if f.first_ts == 0 { ts } else { f.first_ts.min(ts) };
                    f.last_ts = f.last_ts.max(ts);
                    if event == "PreToolUse" {
                        if decision == "block" {
                            f.denies += 1;
                            *f.blocks_by_rule.entry(rule).or_default() += 1;
                        }
                        continue;
                    }
                    f.stops += 1;
                    match decision.as_str() {
                        "block" => {
                            f.near_misses += 1;
                            *f.blocks_by_rule.entry(rule).or_default() += 1;
                        }
                        "allow" => f.allows += 1,
                        "softened" => f.softened += 1,
                        "capped" => f.capped += 1,
                        "override" => f.overrides += 1,
                        _ => {}
                    }
                }
                Line::Claim { class, .. } => *f.claims_by_class.entry(class).or_default() += 1,
                Line::Run { runner, outcome, .. } => {
                    *f.runs_by_runner.entry(runner).or_default() += 1;
                    match outcome.as_str() {
                        "pass" => f.runs_pass += 1,
                        "fail" => f.runs_fail += 1,
                        _ => f.runs_unknown += 1,
                    }
                }
                _ => {}
            }
        }
    }
    f
}

pub fn near_miss_rate(f: &FleetStats) -> Option<f64> {
    (f.stops > 0).then(|| f.near_misses as f64 / f.stops as f64)
}

/// The shareable blob: counters plus a salted hash of the repo root so multiple submissions
/// from one repo can be de-duplicated without revealing where the repo lives.
pub fn blob(root: &Path, f: &FleetStats) -> serde_json::Value {
    let mut v = serde_json::to_value(f).expect("counters serialize");
    let o = v.as_object_mut().expect("object");
    o.insert("schema".into(), "provalot-share/1".into());
    o.insert("provalot".into(), env!("CARGO_PKG_VERSION").into());
    o.insert(
        "repo".into(),
        repo::sha256_str(&format!("provalot-share:{}", root.display()))[..12].into(),
    );
    v
}

fn baseline() -> serde_json::Value {
    serde_json::from_str(BASELINE).unwrap_or(serde_json::Value::Null)
}

fn pct(x: f64) -> String {
    format!("{:.2}%", x * 100.0)
}

/// `provalot share`: benchmark first (the reporter gets the mirror), then the blob to share.
pub fn render_share(root: &Path) -> String {
    let f = collect(root);
    let b = baseline();
    let mut out = String::new();
    match near_miss_rate(&f) {
        Some(r) => {
            out.push_str(&format!(
                "Your agents' unbacked-claim rate: {} ({} near misses in {} evaluated stops)\n",
                pct(r),
                f.near_misses,
                f.stops
            ));
        }
        None => out.push_str("No evaluated stops recorded yet in this repo.\n"),
    }
    if let (Some(m), Some(n)) = (b["near_miss_rate_median"].as_f64(), b["fleets"].as_u64()) {
        out.push_str(&format!(
            "Corpus median: {} (corpus of {} fleet{} so far — {})\n",
            pct(m),
            n,
            if n == 1 { "" } else { "s" },
            b["source"].as_str().unwrap_or("unknown source")
        ));
    }
    out.push_str("\nDe-identified blob (counts only; hashed repo id; no commands, paths or text):\n");
    out.push_str(&serde_json::to_string_pretty(&blob(root, &f)).unwrap_or_default());
    out.push_str("\n\nSharing is optional and manual: paste the blob at https://github.com/prarambh-labs/provalot/discussions if you want it in the corpus. provalot never transmits anything itself.\n");
    out
}

/// `provalot digest`: a Callback-style anonymized fire-pattern digest for this repo.
pub fn render_digest(root: &Path) -> String {
    let f = collect(root);
    let mut out = String::from("# provalot digest — near misses and fire patterns\n\n");
    out.push_str(&format!(
        "Sessions: {}. Evaluated stops: {}. **Near misses: {}** — unbacked claims stopped before they shipped",
        f.sessions, f.stops, f.near_misses
    ));
    if let Some(r) = near_miss_rate(&f) {
        out.push_str(&format!(" ({} of stops)", pct(r)));
    }
    out.push_str(".\n\n## What agents claimed\n\n");
    if f.claims_by_class.is_empty() {
        out.push_str("(no claims recorded)\n");
    }
    for (class, n) in &f.claims_by_class {
        out.push_str(&format!("- {class}: {n}\n"));
    }
    out.push_str("\n## What the ledger could not back\n\n");
    if f.blocks_by_rule.is_empty() {
        out.push_str("(no blocks or denies)\n");
    }
    for (rule, n) in &f.blocks_by_rule {
        out.push_str(&format!("- {rule}: {n}\n"));
    }
    out.push_str(&format!(
        "\nRetries softened after new activity: {}. Human overrides: {}. Cap reached: {}. Policy denies: {}.\n",
        f.softened, f.overrides, f.capped, f.denies
    ));
    out.push_str("\n## What actually ran\n\n| runner | runs |\n|---|---|\n");
    for (runner, n) in &f.runs_by_runner {
        out.push_str(&format!("| {runner} | {n} |\n"));
    }
    out.push_str(&format!(
        "\nOutcomes: {} pass / {} fail / {} unknown.\n\nCounts only — no commands, paths or message text appear in this digest.\n",
        f.runs_pass, f.runs_fail, f.runs_unknown
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let lines = [
            Line::Run {
                ts: 1,
                agent_id: None,
                tool: "Bash".into(),
                command: "run-sekrit-suite --now".into(),
                runner: "pytest".into(),
                outcome: "pass".into(),
                stdout_hash: String::new(),
                stderr_hash: String::new(),
                is_error: false,
                interrupted: false,
            },
            Line::Edit {
                ts: 2,
                agent_id: None,
                tool: "Edit".into(),
                path: "sekrit/file.py".into(),
                hash_before: None,
                hash_after: None,
                changed: true,
            },
            Line::Claim {
                ts: 3,
                agent_id: None,
                event: "Stop".into(),
                class: "tests-pass".into(),
                text: "sekrit tests pass".into(),
                path: Some("sekrit/file.py".into()),
            },
            Line::Decision {
                ts: 4,
                agent_id: None,
                event: "Stop".into(),
                decision: "block".into(),
                rule: "R1.tests-claimed-not-run".into(),
                reason: "[provalot] sekrit reason".into(),
                consecutive: 1,
            },
            Line::Decision {
                ts: 5,
                agent_id: None,
                event: "Stop".into(),
                decision: "allow".into(),
                rule: String::new(),
                reason: String::new(),
                consecutive: 0,
            },
        ];
        for l in &lines {
            ledger::append(dir.path(), "s1", l).unwrap();
        }
        dir
    }

    #[test]
    fn nothing_from_the_ledger_text_leaks() {
        let dir = seeded_root();
        for rendered in [
            serde_json::to_string(&blob(dir.path(), &collect(dir.path()))).unwrap(),
            render_digest(dir.path()),
        ] {
            assert!(!rendered.contains("sekrit"), "{rendered}");
            assert!(
                !rendered.contains(dir.path().to_str().unwrap()),
                "repo path leaked"
            );
        }
    }

    #[test]
    fn counts_and_rate_are_right() {
        let dir = seeded_root();
        let f = collect(dir.path());
        assert_eq!((f.sessions, f.stops, f.near_misses, f.allows), (1, 2, 1, 1));
        assert_eq!(near_miss_rate(&f), Some(0.5));
        assert_eq!(f.blocks_by_rule["R1.tests-claimed-not-run"], 1);
        assert_eq!(f.claims_by_class["tests-pass"], 1);
        assert_eq!((f.runs_pass, f.runs_by_runner["pytest"]), (1, 1));
    }

    #[test]
    fn share_benchmarks_against_the_embedded_baseline() {
        let dir = seeded_root();
        let out = render_share(dir.path());
        assert!(out.contains("unbacked-claim rate: 50.00%"), "{out}");
        assert!(out.contains("Corpus median: 0.62%"), "{out}");
        assert!(out.contains("provalot-share/1"));
        assert!(out.contains("never transmits"));
    }
}
