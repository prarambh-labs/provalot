use crate::adapters;
use crate::claims;
use crate::event::{Event, Harness};
use crate::output;

pub struct HookOutcome {
    pub stdout: Option<String>,
}

pub fn run(harness: Harness, stdin_json: &str) -> Result<HookOutcome, String> {
    let raw: serde_json::Value = serde_json::from_str(stdin_json).map_err(|e| format!("bad json: {e}"))?;
    let event = adapters::parse(harness, &raw)?;
    Ok(match event {
        Event::Stop { last_message, .. } if claims::has_tests_pass_claim(&last_message) => HookOutcome {
            stdout: Some(output::stop_block(
                "[provalot] Claimed tests pass, but no test runner has passed in this session. Run the tests now, or say they were not run.",
            )),
        },
        _ => HookOutcome { stdout: None },
    })
}
