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
            eprintln!("provalot: hook for {harness} not implemented yet");
            std::process::exit(0);
        }
    }
}
