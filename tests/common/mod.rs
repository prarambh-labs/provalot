#![allow(dead_code)]
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

pub fn run(args: &[&str], stdin: &str, cwd: &Path, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_provalot"));
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn provalot");
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().expect("wait for provalot")
}

pub fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

/// Reads fixtures/hooks/<name> and substitutes __CWD__ (quoted or inside a string) with `cwd`.
pub fn fixture(name: &str, cwd: &Path) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/hooks")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    let cwd_json = serde_json::to_string(cwd.to_str().unwrap()).unwrap();
    let bare = cwd_json.trim_matches('"').to_string();
    text.replace("\"__CWD__\"", &cwd_json).replace("__CWD__", &bare)
}

pub fn ledger_lines(cwd: &Path, session: &str) -> Vec<serde_json::Value> {
    let p = cwd.join(".provalot/sessions").join(format!("{session}.jsonl"));
    let Ok(text) = std::fs::read_to_string(&p) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

pub fn sleep_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
