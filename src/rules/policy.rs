use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::evidence::runner::{self, Runner};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    Commit,
    Push,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyRule {
    DenyCommand {
        tokens: Vec<String>,
        source: String,
    },
    ProtectPath {
        prefix: String,
        source: String,
    },
    RequireBefore {
        gate: Gate,
        runner: Option<Runner>,
        source: String,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Policy {
    pub rules: Vec<PolicyRule>,
    pub advisory: Vec<String>,
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}

re!(
    NEVER_RUN,
    r"(?i)^\s*(?:[-*]\s*)?(?:\*\*)?(?:never|do not|don't|must not)(?:\*\*)?\s+(?:run|execute|use)\s+`?([^`\n]+?)`?\s*\.?\s*$"
);
re!(
    PROTECT,
    r"(?i)^\s*(?:[-*]\s*)?(?:\*\*)?(?:never|do not|don't|must not)(?:\*\*)?\s+(?:edit|touch|modify|change)\s+`?([A-Za-z0-9_./\\-]+)`?(?:\s+without\s+asking)?\s*\.?\s*$"
);
re!(
    REQUIRE,
    r"(?i)^\s*(?:[-*]\s*)?(?:\*\*)?(?:always|must)(?:\*\*)?\s+run\s+(?:the\s+)?`?([^`\n]+?)`?\s+before\s+(?:you\s+)?(?:committing|commit|pushing|push)\b"
);
re!(
    DIRECTIVE,
    r"(?i)^\s*(?:[-*]\s*)?(?:\*\*)?(?:must|never|always|do not|don't)\b"
);

pub fn compile(text: &str) -> Policy {
    let mut p = Policy::default();
    for line in text.lines() {
        let source = line.trim().to_string();
        if let Some(c) = NEVER_RUN.captures(line) {
            let tokens = c[1].split_whitespace().map(|s| s.to_string()).collect();
            p.rules.push(PolicyRule::DenyCommand { tokens, source });
        } else if let Some(c) = PROTECT.captures(line) {
            p.rules.push(PolicyRule::ProtectPath {
                prefix: c[1].trim_end_matches('/').to_string(),
                source,
            });
        } else if let Some(c) = REQUIRE.captures(line) {
            let what = c[1].trim().to_lowercase();
            let gate = if line.to_lowercase().contains("push") {
                Gate::Push
            } else {
                Gate::Commit
            };
            let generic = matches!(
                what.as_str(),
                "test suite" | "tests" | "the tests" | "test" | "all tests"
            );
            let runner = if generic {
                None
            } else {
                match runner::classify(&what) {
                    Runner::Other => {
                        p.advisory.push(source);
                        continue;
                    }
                    r => Some(r),
                }
            };
            p.rules.push(PolicyRule::RequireBefore { gate, runner, source });
        } else if DIRECTIVE.is_match(line) {
            p.advisory.push(source);
        }
    }
    p
}

use crate::ledger::Line;
use crate::rules::r1_tests;
use crate::rules::Block;

pub fn contains_subsequence(hay: &[String], needle: &[String]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

fn gate_tokens(gate: &Gate) -> Vec<String> {
    match gate {
        Gate::Commit => vec!["git".into(), "commit".into()],
        Gate::Push => vec!["git".into(), "push".into()],
    }
}

pub fn check_command(policy: &Policy, command: &str, lines: &[Line]) -> Option<Block> {
    for seg in runner::segment_tokens(command) {
        for rule in &policy.rules {
            match rule {
                PolicyRule::DenyCommand { tokens, source } if contains_subsequence(&seg, tokens) => {
                    return Some(Block {
                        rule: "R3.deny-command",
                        reason: format!(
                            "[provalot] Blocked by policy \"{source}\": the command contains `{}`. Do not run it.",
                            tokens.join(" ")
                        ),
                    });
                }
                PolicyRule::RequireBefore { gate, runner, source }
                    if contains_subsequence(&seg, &gate_tokens(gate))
                        && !r1_tests::evidence_current(lines, *runner) =>
                {
                    let what = runner
                        .map(|r| r.as_str().to_string())
                        .unwrap_or_else(|| "the tests".into());
                    return Some(Block {
                        rule: "R3.require-before",
                        reason: format!(
                            "[provalot] Policy \"{source}\": no passing run of {what} is recorded since the last edit. Run {what} first, then retry."
                        ),
                    });
                }
                _ => {}
            }
        }
    }
    None
}

pub fn check_edit(policy: &Policy, rel_path: &str) -> Option<Block> {
    for rule in &policy.rules {
        if let PolicyRule::ProtectPath { prefix, source } = rule {
            if rel_path == prefix || rel_path.starts_with(&format!("{prefix}/")) {
                return Some(Block {
                    rule: "R3.protect-path",
                    reason: format!(
                        "[provalot] Policy \"{source}\": {rel_path} is protected. Ask the user; they can run `provalot allow --once` to permit this edit."
                    ),
                });
            }
        }
    }
    None
}

pub fn load(root: &Path) -> Policy {
    let mut text = String::new();
    for name in ["CLAUDE.md", "AGENTS.md", ".claude/CLAUDE.md"] {
        if let Ok(t) = std::fs::read_to_string(root.join(name)) {
            text.push_str(&t);
            text.push('\n');
        }
    }
    compile(&text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::Line;

    #[test]
    fn subsequence_matching() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(contains_subsequence(
            &s(&["git", "push", "--force", "origin"]),
            &s(&["git", "push", "--force"])
        ));
        assert!(!contains_subsequence(
            &s(&["git", "push", "origin", "--force"]),
            &s(&["git", "push", "--force"])
        ));
        assert!(!contains_subsequence(&s(&["git"]), &s(&[])));
    }

    #[test]
    fn check_command_and_edit() {
        let text = std::fs::read_to_string(format!(
            "{}/fixtures/policy/CLAUDE.md",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let p = compile(&text);
        assert_eq!(
            check_command(&p, "git push --force origin main", &[])
                .unwrap()
                .rule,
            "R3.deny-command"
        );
        assert_eq!(
            check_command(&p, "rtk git push --force", &[]).unwrap().rule,
            "R3.deny-command"
        );
        assert_eq!(
            check_command(&p, "git commit -m x", &[]).unwrap().rule,
            "R3.require-before"
        );
        let pass = Line::Run {
            ts: 5,
            agent_id: None,
            tool: "Bash".into(),
            command: "pytest".into(),
            runner: "pytest".into(),
            outcome: "pass".into(),
            stdout_hash: String::new(),
            stderr_hash: String::new(),
            is_error: false,
            interrupted: false,
        };
        assert!(check_command(&p, "git commit -m x", &[pass]).is_none());
        assert!(check_command(&p, "ls -la", &[]).is_none());
        assert!(
            check_command(&p, "git push origin main", &[]).is_none(),
            "push is not gated by this policy"
        );
        assert_eq!(
            check_edit(&p, "migrations/0002_add.sql").unwrap().rule,
            "R3.protect-path"
        );
        assert!(check_edit(&p, "migrations_old/x.sql").is_none());
        assert!(check_edit(&p, "src/app.py").is_none());
    }

    #[test]
    fn compiles_the_three_shapes_and_reports_the_rest() {
        let text = std::fs::read_to_string(format!(
            "{}/fixtures/policy/CLAUDE.md",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let p = compile(&text);
        assert_eq!(p.rules.len(), 3, "{:?}", p.rules);
        assert!(
            matches!(&p.rules[0], PolicyRule::DenyCommand { tokens, .. } if tokens == &["git", "push", "--force"])
        );
        assert!(matches!(
            &p.rules[1],
            PolicyRule::RequireBefore {
                gate: Gate::Commit,
                runner: None,
                ..
            }
        ));
        assert!(matches!(&p.rules[2], PolicyRule::ProtectPath { prefix, .. } if prefix == "migrations"));
        assert_eq!(p.advisory, vec!["- **MUST** commit after every task."]);
    }

    #[test]
    fn require_before_with_a_named_runner_and_push() {
        let p = compile("ALWAYS run `cargo test` before pushing");
        assert!(matches!(
            &p.rules[0],
            PolicyRule::RequireBefore {
                gate: Gate::Push,
                runner: Some(Runner::Cargo),
                ..
            }
        ));
        let p = compile("ALWAYS run `make lint` before committing");
        assert!(p.rules.is_empty());
        assert_eq!(p.advisory.len(), 1, "unrecognized runner becomes advisory");
    }

    #[test]
    fn load_reads_all_three_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "NEVER run rm -rf /\n").unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
        std::fs::write(dir.path().join(".claude/CLAUDE.md"), "Do not edit vendor/\n").unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "MUST use pnpm\n").unwrap();
        let p = load(dir.path());
        assert_eq!(p.rules.len(), 2);
        assert_eq!(p.advisory.len(), 1);
    }
}
