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
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            match provalot::hook::run(harness, &input) {
                Ok(out) => {
                    if let Some(s) = out.stdout {
                        println!("{s}");
                    }
                }
                Err(e) => eprintln!("provalot: {e}"),
            }
            std::process::exit(0);
        }
    }
}
