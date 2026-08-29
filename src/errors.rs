use std::io::Write;
use std::path::Path;

/// Append one line to `.provalot/errors.log`. Never fails loudly.
pub fn log(root: &Path, msg: &str) {
    let p = root.join(".provalot").join("errors.log");
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(
            f,
            "{} {}",
            crate::ledger::now_ms(),
            msg.lines().next().unwrap_or("")
        );
    }
}
