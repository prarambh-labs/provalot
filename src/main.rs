use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "provalot",
    version,
    about = "Deterministic evidence gate for coding agents"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Hook entry point: reads the harness event JSON on stdin (harness: claude | codex)
    Hook { harness: String },
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Hook { harness } => {
            use std::io::Read;
            let harness = match harness.as_str() {
                "claude" => provalot::event::Harness::Claude,
                "codex" => provalot::event::Harness::Codex,
                _ => std::process::exit(0),
            };
            std::panic::set_hook(Box::new(|_| {}));
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let root = provalot::repo::find_root(&cwd);
            match std::panic::catch_unwind(|| provalot::hook::run(harness, &input)) {
                Ok(Ok(out)) => {
                    if let Some(s) = out.stdout {
                        println!("{s}");
                    }
                }
                Ok(Err(e)) => provalot::errors::log(&root, &e),
                Err(_) => provalot::errors::log(&root, "panic in hook"),
            }
            std::process::exit(0);
        }
    }
}
