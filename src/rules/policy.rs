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
