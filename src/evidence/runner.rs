#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    Pytest,
    Unittest,
    Cargo,
    NpmTest,
    Vitest,
    Jest,
    NodeTest,
    GoTest,
    Xcodebuild,
    SwiftTest,
    /// A project's own test entry point: a test-named script or a build tool's `test` task.
    Script,
    Other,
}

impl Runner {
    pub fn as_str(self) -> &'static str {
        match self {
            Runner::Pytest => "pytest",
            Runner::Unittest => "unittest",
            Runner::Cargo => "cargo",
            Runner::NpmTest => "npm-test",
            Runner::Vitest => "vitest",
            Runner::Jest => "jest",
            Runner::NodeTest => "node-test",
            Runner::GoTest => "go-test",
            Runner::Xcodebuild => "xcodebuild",
            Runner::SwiftTest => "swift-test",
            Runner::Script => "script",
            Runner::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Runner {
        match s {
            "pytest" => Runner::Pytest,
            "unittest" => Runner::Unittest,
            "cargo" => Runner::Cargo,
            "npm-test" => Runner::NpmTest,
            "vitest" => Runner::Vitest,
            "jest" => Runner::Jest,
            "node-test" => Runner::NodeTest,
            "go-test" => Runner::GoTest,
            "xcodebuild" => Runner::Xcodebuild,
            "swift-test" => Runner::SwiftTest,
            "script" => Runner::Script,
            _ => Runner::Other,
        }
    }
}

/// Split on `&&`, `||`, `|`, `;` and newlines. Quotes are not parsed; good enough for runner detection.
fn segments(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '&' | '|' if chars.peek() == Some(&c) => {
                chars.next();
                out.push(std::mem::take(&mut cur));
            }
            '|' | ';' | '\n' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Drop leading wrappers (`rtk`, `time`, `nice`, `timeout <n>`, `sudo`, `env` and their
/// flags) and `VAR=value` assignments.
fn strip_wrappers(tokens: &[String]) -> Vec<String> {
    let mut i = 0;
    while let Some(t) = tokens.get(i) {
        if matches!(t.as_str(), "rtk" | "time" | "nice" | "sudo" | "env" | "timeout") {
            let is_timeout = t == "timeout";
            i += 1;
            while tokens.get(i).map(|x| x.starts_with('-')).unwrap_or(false) {
                i += 1;
            }
            // `timeout [-k n] <duration> cmd`: durations are numeric, commands are not.
            while is_timeout
                && tokens
                    .get(i)
                    .map(|x| x.starts_with(|c: char| c.is_ascii_digit()))
                    .unwrap_or(false)
            {
                i += 1;
            }
            continue;
        }
        if t.contains('=') && !t.starts_with('-') && !t.contains('/') {
            i += 1;
            continue;
        }
        break;
    }
    tokens[i..].to_vec()
}

fn split_tokens(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .map(|s| s.trim_matches(|c| c == '\'' || c == '"').to_string())
        .collect()
}

/// Tokens of the first segment with wrappers stripped (used for policy matching).
pub fn tokens(command: &str) -> Vec<String> {
    segments(command)
        .first()
        .map(|s| strip_wrappers(&split_tokens(s)))
        .unwrap_or_default()
}

/// Tokens of every segment, both wrapper-stripped and verbatim.
///
/// A policy may name a wrapper it denies (`NEVER run sudo rm -rf`), so those rules have to be
/// matched against the unstripped tokens too, or they could never fire.
pub fn segment_tokens(command: &str) -> Vec<(Vec<String>, Vec<String>)> {
    segments(command)
        .iter()
        .map(|s| {
            let raw = split_tokens(s);
            (strip_wrappers(&raw), raw)
        })
        .collect()
}

fn classify_tokens(t: &[String]) -> Runner {
    let Some(first) = t.first() else {
        return Runner::Other;
    };
    let first = first.rsplit('/').next().unwrap_or(first).to_string();
    let rest: Vec<&str> = t.iter().skip(1).map(|s| s.as_str()).collect();
    let has_m_pytest = rest.windows(2).any(|w| w[0] == "-m" && w[1] == "pytest");
    let has_m_unittest = rest.windows(2).any(|w| w[0] == "-m" && w[1] == "unittest");
    match first.as_str() {
        "pytest" | "py.test" => Runner::Pytest,
        "python" | "python3" | "uv" | "poetry" | "pipenv" if has_m_pytest || rest.contains(&"pytest") => {
            Runner::Pytest
        }
        "python" | "python3" | "uv" | "poetry" | "pipenv" if has_m_unittest => Runner::Unittest,
        "cargo" if matches!(rest.first(), Some(&"test") | Some(&"nextest")) => Runner::Cargo,
        "npm" | "pnpm" | "yarn" | "bun"
            if matches!(rest.first(), Some(&"test") | Some(&"t"))
                || (rest.first() == Some(&"run")
                    && rest.get(1).map(|s| s.starts_with("test")).unwrap_or(false)) =>
        {
            Runner::NpmTest
        }
        "npx" | "pnpx" | "bunx" => match rest.first() {
            Some(&"vitest") => Runner::Vitest,
            Some(&"jest") => Runner::Jest,
            _ => Runner::Other,
        },
        "vitest" => Runner::Vitest,
        "jest" => Runner::Jest,
        "node" if rest.contains(&"--test") => Runner::NodeTest,
        "go" if rest.first() == Some(&"test") => Runner::GoTest,
        "xcodebuild" if rest.contains(&"test") || rest.contains(&"test-without-building") => {
            Runner::Xcodebuild
        }
        "swift" if rest.first() == Some(&"test") => Runner::SwiftTest,
        "make" | "just" | "gradle" | "gradlew" | "mvn" | "dotnet" | "mix" | "sbt" | "meson"
            if rest
                .first()
                .is_some_and(|a| a.starts_with("test") || *a == "check") =>
        {
            Runner::Script
        }
        "tox" | "nox" | "rspec" | "phpunit" | "ctest" | "busted" | "bats" => Runner::Script,
        "bash" | "sh" | "zsh" | "python" | "python3" | "node" | "ruby" | "perl" | "bun" | "deno"
            if rest
                .iter()
                .find(|a| !a.starts_with('-'))
                .is_some_and(|a| test_named(a)) =>
        {
            Runner::Script
        }
        _ if test_named(&first) => Runner::Script,
        _ => Runner::Other,
    }
}

/// `test.sh`, `tools/test_claude_service.sh`, `run_tests.py`, `scripts/run-tests`, `tests/smoke_test`.
/// The executable itself must be test-named; a file argument (`cat test.log`) does not count.
fn test_named(token: &str) -> bool {
    let base = token.rsplit('/').next().unwrap_or(token);
    let stem = base.split('.').next().unwrap_or(base).to_ascii_lowercase();
    stem.split(['_', '-'])
        .any(|w| matches!(w, "test" | "tests" | "selftest" | "check"))
}

/// First recognized test runner in any segment of the command line, else `Other`.
pub fn classify(command: &str) -> Runner {
    for seg in segments(command) {
        let r = classify_tokens(&strip_wrappers(&split_tokens(&seg)));
        if r != Runner::Other {
            return r;
        }
    }
    Runner::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_commands() {
        let cases = [
            ("pytest -q", Runner::Pytest),
            ("rtk pytest tests/", Runner::Pytest),
            ("cd api && python -m pytest -x", Runner::Pytest),
            ("FOO=1 pytest", Runner::Pytest),
            ("uv run pytest", Runner::Pytest),
            ("python3 -m unittest discover -s tests -q", Runner::Unittest),
            ("python -m unittest tests.test_app", Runner::Unittest),
            (
                "a && python3 -m unittest discover -s tests -q; b",
                Runner::Unittest,
            ),
            ("cd api && rtk python3 -m unittest -v", Runner::Unittest),
            ("cargo test --workspace", Runner::Cargo),
            ("rtk cargo test", Runner::Cargo),
            ("cargo nextest run", Runner::Cargo),
            ("npm test", Runner::NpmTest),
            ("npm run test:unit -- --ci", Runner::NpmTest),
            ("pnpm t", Runner::NpmTest),
            ("npx vitest run", Runner::Vitest),
            ("npx jest src/", Runner::Jest),
            ("node --test", Runner::NodeTest),
            ("go test ./...", Runner::GoTest),
            (
                "xcodebuild -scheme App -destination 'platform=iOS Simulator' test",
                Runner::Xcodebuild,
            ),
            ("swift test", Runner::SwiftTest),
            ("tools/test_claude_service.sh", Runner::Script),
            ("bash tools/test_claude_service.sh --verbose", Runner::Script),
            ("./test.sh", Runner::Script),
            ("python3 tests/run_tests.py", Runner::Script),
            ("scripts/run-tests", Runner::Script),
            ("make test", Runner::Script),
            ("just test-unit", Runner::Script),
            ("./gradlew test", Runner::Script),
            ("dotnet test", Runner::Script),
            ("bundle exec rspec", Runner::Other),
            ("rspec spec/", Runner::Script),
            ("cat test.log", Runner::Other),
            ("vim tests/test_foo.py", Runner::Other),
            ("make build", Runner::Other),
            ("ls -la", Runner::Other),
            ("git commit -m 'x'", Runner::Other),
            ("cargo build", Runner::Other),
            ("npm run build", Runner::Other),
            ("echo pytest", Runner::Other),
            ("timeout 60 pytest", Runner::Pytest),
            ("timeout -k 5 300 cargo test", Runner::Cargo),
            ("sudo -E env CI=1 cargo test", Runner::Cargo),
            ("env NODE_ENV=test npm test", Runner::NpmTest),
        ];
        for (cmd, want) in cases {
            assert_eq!(classify(cmd), want, "command: {cmd}");
        }
    }

    #[test]
    fn tokens_strip_wrappers() {
        assert_eq!(
            tokens("rtk --ultra-compact git push --force"),
            vec!["git", "push", "--force"]
        );
        assert_eq!(
            tokens("CI=1 time git commit -m x && ls"),
            vec!["git", "commit", "-m", "x"]
        );
    }

    #[test]
    fn runner_round_trips() {
        for r in [
            Runner::Pytest,
            Runner::Unittest,
            Runner::Cargo,
            Runner::NpmTest,
            Runner::Vitest,
            Runner::Jest,
            Runner::NodeTest,
            Runner::GoTest,
            Runner::Xcodebuild,
            Runner::SwiftTest,
            Runner::Script,
            Runner::Other,
        ] {
            assert_eq!(Runner::parse(r.as_str()), r);
        }
    }
}
