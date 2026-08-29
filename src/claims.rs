use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimClass {
    TestsPass,
    FileEdited,
}

impl ClaimClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaimClass::TestsPass => "tests-pass",
            ClaimClass::FileEdited => "file-edited",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub class: ClaimClass,
    pub text: String,
    pub path: Option<String>,
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}

re!(INLINE_CODE, r"`[^`\n]*`");
re!(
    TESTS_PASS,
    r"(?i)\b(?:all\s+)?(?:the\s+)?tests?\s+(?:are\s+|now\s+)*(?:pass(?:es|ing|ed)?|green)\b"
);
re!(
    ALL_PASS,
    r"(?i)\ball\s+(?:\d+\s+)?(?:tests?|specs?|checks?)\s+(?:are\s+)?(?:pass(?:ing|ed|es)?|green)\b"
);
re!(
    SUITE,
    r"(?i)\b(?:test\s+)?suite\s+(?:is\s+|now\s+)*(?:pass(?:es|ing|ed)?|green|clean)\b"
);
re!(GREEN, r"(?i)\b(?:everything|all)\s+(?:is\s+)?green\b");
re!(N_OF_M, r"(?i)\b\d+\s*/\s*\d+\s+(?:tests?\s+)?pass(?:ing|ed)?\b");
re!(
    FILE_EDITED,
    r"(?i)\b(?:updated|edited|modified|changed|wrote|rewrote|patched|touched|refactored)\s+(?:the\s+)?(?:file\s+)?([A-Za-z0-9_./\\-]+\.[A-Za-z0-9]+)"
);

const NEGATORS: &[&str] = &[
    "not",
    "no",
    "none",
    "nothing",
    "never",
    "neither",
    "nor",
    "without",
    "cannot",
    "cant",
    "fail",
    "fails",
    "failed",
    "failing",
    "broken",
    "breaks",
    "unresolved",
    "incomplete",
    "unfinished",
    "dont",
    "doesnt",
    "didnt",
    "isnt",
    "arent",
    "wasnt",
    "werent",
    "havent",
    "hasnt",
    "wont",
    "may",
    "might",
];

const HEDGES: &[&str] = &[
    "almost",
    "nearly",
    "mostly",
    "partially",
    "partly",
    "should",
    "would",
    "will",
    "going",
    "need",
    "needs",
    "once",
    "after",
    "until",
    "unless",
    "if",
    "when",
    "hopefully",
    "expect",
    "expected",
    "assuming",
    "next",
];

/// Drops fenced code, inline code, blockquotes, table rows, questions, and our own block reasons.
pub fn preprocess(message: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in message.lines() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence
            || t.starts_with('>')
            || t.starts_with('|')
            || t.ends_with('?')
            || t.contains("[provalot]")
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    INLINE_CODE.replace_all(&out, " ").to_string()
}

fn words_before(text: &str, end: usize, n: usize) -> Vec<String> {
    text[..end]
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase().replace('\'', ""))
        .rev()
        .take(n)
        .collect()
}

fn words_after(text: &str, start: usize, n: usize) -> Vec<String> {
    text[start..]
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .take(n)
        .collect()
}

fn vetoed(text: &str, start: usize, end: usize) -> bool {
    let before = words_before(text, start, 4);
    let after = words_after(text, end, 2);
    before
        .iter()
        .any(|w| NEGATORS.contains(&w.as_str()) || HEDGES.contains(&w.as_str()))
        || after.iter().any(|w| w == "yet")
}

pub fn extract(message: &str, root: &Path) -> Vec<Claim> {
    let text = preprocess(message);
    let mut claims = Vec::new();

    let mut spans: Vec<(usize, usize)> = Vec::new();
    for re in [&*TESTS_PASS, &*ALL_PASS, &*SUITE, &*GREEN, &*N_OF_M] {
        for m in re.find_iter(&text) {
            spans.push((m.start(), m.end()));
        }
    }
    spans.sort();
    let mut last_end = 0;
    for (s, e) in spans {
        if s < last_end {
            continue;
        }
        last_end = e;
        if vetoed(&text, s, e) {
            continue;
        }
        claims.push(Claim {
            class: ClaimClass::TestsPass,
            text: text[s..e].to_string(),
            path: None,
        });
    }

    for c in FILE_EDITED.captures_iter(&text) {
        let whole = c.get(0).unwrap();
        let token = c
            .get(1)
            .unwrap()
            .as_str()
            .trim_matches(|ch: char| matches!(ch, '.' | ',' | ')' | '('));
        let candidate = root.join(token);
        if !candidate.is_file() || vetoed(&text, whole.start(), whole.end()) {
            continue;
        }
        claims.push(Claim {
            class: ClaimClass::FileEdited,
            text: whole.as_str().to_string(),
            path: Some(crate::repo::rel(root, &candidate)),
        });
    }
    claims
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            let p = dir.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "x").unwrap();
        }
        dir
    }

    fn tests_pass_count(msg: &str) -> usize {
        let dir = root_with(&[]);
        extract(msg, dir.path())
            .iter()
            .filter(|c| c.class == ClaimClass::TestsPass)
            .count()
    }

    #[test]
    fn positive_test_claims() {
        for m in [
            "All tests pass.",
            "Tests are passing now.",
            "The test suite is green.",
            "12/12 tests passing.",
            "Everything is green, ready for review.",
            "Ran pytest: 4 passed. All tests pass.",
        ] {
            assert_eq!(tests_pass_count(m), 1, "{m}");
        }
    }

    #[test]
    fn negated_hedged_and_quoted_claims_are_not_claims() {
        for m in [
            "Tests don't pass yet.",
            "The tests should pass once you run them.",
            "I have not run the tests; they may pass.",
            "```\n4 passed in 0.1s\nall tests pass\n```\nI did not run anything.",
            "| tests pass | expected |",
            "Are all tests passing?",
            "Done when all tests pass.",
            "Run `pytest` until all tests pass.",
            "> all tests pass",
            "[provalot] Claimed tests pass, but no test runner has passed.",
        ] {
            assert_eq!(tests_pass_count(m), 0, "{m}");
        }
    }

    #[test]
    fn file_edited_claims_need_an_existing_path() {
        let dir = root_with(&["src/app.py", "README.md"]);
        let claims = extract(
            "I updated src/app.py and rewrote README.md. Also edited docs/missing.md.",
            dir.path(),
        );
        let paths: Vec<&str> = claims.iter().filter_map(|c| c.path.as_deref()).collect();
        assert_eq!(paths, vec!["src/app.py", "README.md"]);
        assert!(extract("I haven't updated src/app.py yet.", dir.path()).is_empty());
        assert!(extract("Next I will update src/app.py.", dir.path()).is_empty());
    }

    #[test]
    fn preprocess_strips_fences_inline_code_quotes_tables_and_questions() {
        let out = preprocess("a\n```\nfenced\n```\nb `inline` c\n> quoted\n| t |\nwhy?\nd");
        assert_eq!(out, "a\nb   c\nd\n");
    }
}
