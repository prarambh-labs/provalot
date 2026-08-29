use std::path::{Path, PathBuf};

use crate::ledger::{self, Line};

pub fn render(root: &Path, session: &str) -> String {
    let lines = ledger::read(root, session);
    let mut out = format!("# provalot report: session {session}\n\n");
    out.push_str("## Runs\n\n| runner | outcome | command |\n|---|---|---|\n");
    for l in &lines {
        if let Line::Run {
            runner,
            outcome,
            command,
            ..
        } = l
        {
            out.push_str(&format!(
                "| {runner} | {outcome} | `{}` |\n",
                command.replace('|', "\\|")
            ));
        }
    }
    out.push_str("\n## Edits\n\n| path | changed |\n|---|---|\n");
    for l in &lines {
        if let Line::Edit { path, changed, .. } = l {
            out.push_str(&format!("| {path} | {changed} |\n"));
        }
    }
    out.push_str("\n## Claims\n\n");
    for l in &lines {
        if let Line::Claim {
            class, text, path, ..
        } = l
        {
            out.push_str(&format!(
                "- {class}: \"{text}\"{}\n",
                path.as_ref().map(|p| format!(" ({p})")).unwrap_or_default()
            ));
        }
    }
    out.push_str("\n## Decisions\n\n");
    for l in &lines {
        match l {
            Line::Decision { event, decision, rule, reason, consecutive, .. } => {
                out.push_str(&format!("- {event}: **{decision}** {rule} (consecutive {consecutive}) {reason}\n"));
            }
            Line::Override { .. } => out.push_str("- human override recorded\n"),
            Line::Snapshot { runs, passes, fails, edits_changed, claims, .. } => out.push_str(&format!(
                "- snapshot before compaction: runs {runs} (pass {passes}, fail {fails}), edits changed {edits_changed}, claims {claims}\n"
            )),
            _ => {}
        }
    }
    out
}

pub fn write(root: &Path, session: &str) -> Result<PathBuf, String> {
    let dir = root.join(".provalot").join("reports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join(format!("{}.md", ledger::sanitize(session)));
    std::fs::write(&p, render(root, session)).map_err(|e| e.to_string())?;
    Ok(p)
}
