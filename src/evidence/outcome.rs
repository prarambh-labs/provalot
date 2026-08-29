use std::sync::LazyLock;

use regex::Regex;

use super::runner::Runner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    Unknown,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Fail => "fail",
            Outcome::Unknown => "unknown",
        }
    }
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}

re!(PYTEST_FAIL, r"(?m)\b(\d+) (?:failed|errors?)\b");
re!(PYTEST_PASS, r"(?m)\b(\d+) passed\b");
re!(CARGO_FAIL, r"(?m)^test result: FAILED");
re!(CARGO_PASS, r"(?m)^test result: ok");
re!(
    JS_FAIL,
    r"(?m)^\s*(?:Tests|Test Files|Test Suites):?\s.*?\b(\d+) failed"
);
re!(
    JS_PASS,
    r"(?m)^\s*(?:Tests|Test Files|Test Suites):?\s.*?\b(\d+) passed"
);
re!(NODE_FAIL, r"(?m)^# fail (\d+)");
re!(NODE_PASS, r"(?m)^# pass (\d+)");
// Node >= 20 prints the spec reporter even when stdout is not a TTY.
re!(NODE_SPEC_FAIL, r"(?m)^\s*\x{2139} fail (\d+)");
re!(NODE_SPEC_PASS, r"(?m)^\s*\x{2139} pass (\d+)");
// `rtk` collapses a cargo test run to a single summary line.
re!(RTK_CARGO_FAIL, r"(?m)^cargo test: [^\n]*?\b(\d+) failed\b");
re!(RTK_CARGO_PASS, r"(?m)^cargo test: (\d+) passed\b");
re!(GO_FAIL, r"(?m)^(?:FAIL|--- FAIL)");
re!(GO_PASS, r"(?m)^ok\s");
re!(XC_FAIL, r"\*\* TEST FAILED \*\*");
re!(XC_PASS, r"\*\* TEST SUCCEEDED \*\*");
re!(SWIFT_FAIL, r"(?m)with (\d+) failures?");
re!(SWIFT_PASS, r"(?m)with 0 failures");

/// True when any capture group 1 parses to a number greater than zero (or the regex has no number).
fn nonzero(re: &Regex, text: &str) -> bool {
    re.captures_iter(text).any(|c| {
        c.get(1)
            .map(|m| m.as_str().parse::<u64>().unwrap_or(0) > 0)
            .unwrap_or(true)
    })
}

pub fn infer(runner: Runner, stdout: &str, stderr: &str, is_error: bool, interrupted: bool) -> Outcome {
    if interrupted {
        return Outcome::Fail;
    }
    let text = format!("{stdout}\n{stderr}");
    let js = || (nonzero(&JS_FAIL, &text), nonzero(&JS_PASS, &text));
    let node = || {
        (
            nonzero(&NODE_FAIL, &text) || nonzero(&NODE_SPEC_FAIL, &text),
            nonzero(&NODE_PASS, &text) || nonzero(&NODE_SPEC_PASS, &text),
        )
    };
    let py = || (nonzero(&PYTEST_FAIL, &text), nonzero(&PYTEST_PASS, &text));
    let (fail, pass) = match runner {
        Runner::Pytest => py(),
        Runner::Cargo => (
            CARGO_FAIL.is_match(&text) || nonzero(&RTK_CARGO_FAIL, &text),
            CARGO_PASS.is_match(&text) || nonzero(&RTK_CARGO_PASS, &text),
        ),
        Runner::Vitest | Runner::Jest => js(),
        Runner::NpmTest => {
            let (a, b) = js();
            let (c, d) = node();
            let (e, f) = py();
            (a || c || e, b || d || f)
        }
        Runner::NodeTest => node(),
        Runner::GoTest => (GO_FAIL.is_match(&text), GO_PASS.is_match(&text)),
        Runner::Xcodebuild => (XC_FAIL.is_match(&text), XC_PASS.is_match(&text)),
        Runner::SwiftTest => (nonzero(&SWIFT_FAIL, &text), SWIFT_PASS.is_match(&text)),
        Runner::Other => (false, false),
    };
    if fail || is_error {
        return Outcome::Fail;
    }
    if pass {
        Outcome::Pass
    } else {
        Outcome::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::runner::Runner;

    fn fx(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/fixtures/runner-output/{name}.txt",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn infers_from_real_outputs() {
        let cases = [
            (Runner::Pytest, "pytest-pass", Outcome::Pass),
            (Runner::Pytest, "pytest-fail", Outcome::Fail),
            (Runner::Cargo, "cargo-pass", Outcome::Pass),
            (Runner::Cargo, "cargo-fail", Outcome::Fail),
            (Runner::Cargo, "cargo-rtk-pass", Outcome::Pass),
            (Runner::NodeTest, "node-test-spec-pass", Outcome::Pass),
            (Runner::NodeTest, "node-test-spec-fail", Outcome::Fail),
            (Runner::NpmTest, "node-test-spec-pass", Outcome::Pass),
            (Runner::Jest, "jest-pass", Outcome::Pass),
            (Runner::Jest, "jest-fail", Outcome::Fail),
            (Runner::Vitest, "vitest-pass", Outcome::Pass),
            (Runner::NpmTest, "jest-fail", Outcome::Fail),
            (Runner::NpmTest, "vitest-pass", Outcome::Pass),
            (Runner::NodeTest, "node-test-fail", Outcome::Fail),
            (Runner::GoTest, "go-pass", Outcome::Pass),
            (Runner::GoTest, "go-fail", Outcome::Fail),
            (Runner::Xcodebuild, "xcodebuild-pass", Outcome::Pass),
            (Runner::Xcodebuild, "xcodebuild-fail", Outcome::Fail),
        ];
        for (runner, name, want) in cases {
            assert_eq!(infer(runner, &fx(name), "", false, false), want, "{name}");
        }
    }

    #[test]
    fn flags_override_text() {
        let pass = fx("pytest-pass");
        assert_eq!(
            infer(Runner::Pytest, &pass, "", true, false),
            Outcome::Fail,
            "is_error wins"
        );
        assert_eq!(
            infer(Runner::Pytest, &pass, "", false, true),
            Outcome::Fail,
            "interrupted wins"
        );
        assert_eq!(infer(Runner::Pytest, "", "", false, false), Outcome::Unknown);
        assert_eq!(
            infer(Runner::Other, "anything", "", false, false),
            Outcome::Unknown
        );
        assert_eq!(
            infer(Runner::Pytest, "", "4 passed in 0.1s", false, false),
            Outcome::Pass,
            "stderr is scanned too"
        );
    }

    #[test]
    fn zero_passed_is_not_a_pass() {
        assert_eq!(
            infer(Runner::Pytest, "0 passed, 3 skipped in 0.1s", "", false, false),
            Outcome::Unknown
        );
        assert_eq!(
            infer(
                Runner::NodeTest,
                "# tests 0\n# pass 0\n# fail 0\n",
                "",
                false,
                false
            ),
            Outcome::Unknown
        );
        assert_eq!(
            infer(
                Runner::Cargo,
                "cargo test: 0 passed (0 suites, 0.01s)",
                "",
                false,
                false
            ),
            Outcome::Unknown
        );
    }

    #[test]
    fn rtk_cargo_failure_line_is_a_fail() {
        assert_eq!(
            infer(
                Runner::Cargo,
                "cargo test: 70 passed, 1 failed (19 suites, 2.03s)",
                "",
                false,
                false
            ),
            Outcome::Fail
        );
    }
}
