pub mod r1_tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub rule: &'static str,
    pub reason: String,
}

/// `PROVALOT_DISABLE_RULE=R1,R2` disables rules by id prefix. Exists so selftest can prove the gates gate.
pub fn disabled(rule: &str) -> bool {
    std::env::var("PROVALOT_DISABLE_RULE")
        .map(|v| {
            v.split(',')
                .any(|r| !r.trim().is_empty() && rule.starts_with(r.trim()))
        })
        .unwrap_or(false)
}
