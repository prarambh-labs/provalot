pub mod claude;
pub mod codex;

use crate::event::{Event, Harness};

pub fn parse(harness: Harness, raw: &serde_json::Value) -> Result<Event, String> {
    match harness {
        Harness::Claude => claude::parse(raw),
        Harness::Codex => codex::parse(raw),
    }
}
