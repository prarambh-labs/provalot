use std::sync::LazyLock;

use regex::Regex;

static TESTS_PASS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:all\s+)?(?:the\s+)?tests?\s+(?:are\s+|now\s+)*(?:pass(?:es|ing|ed)?|green)\b")
        .unwrap()
});

pub fn has_tests_pass_claim(message: &str) -> bool {
    TESTS_PASS.is_match(message)
}
