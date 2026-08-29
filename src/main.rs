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
    /// Permit the next blocked decision in the latest session (logged in the report)
    Allow {
        #[arg(long)]
        once: bool,
    },
    /// Install the hooks (default: both harnesses, project scope)
    Init {
        #[arg(long)]
        claude: bool,
        #[arg(long)]
        codex: bool,
        /// Write to ~/.claude/settings.json and ~/.codex/hooks.json instead of the repo
        #[arg(long)]
        user: bool,
    },
    /// Remove the hooks provalot installed
    Uninstall {
        #[arg(long)]
        claude: bool,
        #[arg(long)]
        codex: bool,
        #[arg(long)]
        user: bool,
    },
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
        Cmd::Allow { once } => {
            if !once {
                eprintln!("provalot allow needs --once (v0 has no standing allow)");
                std::process::exit(2);
            }
            let root = root_from_cwd();
            match provalot::decide::record_override(&root) {
                Ok(session) => {
                    println!("override recorded for session {session}: the next block is allowed once")
                }
                Err(e) => {
                    eprintln!("provalot: {e}");
                    std::process::exit(1);
                }
            }
        }
        Cmd::Init { claude, codex, user } => {
            report_lines(provalot::init::init(&root_from_cwd(), claude, codex, user))
        }
        Cmd::Uninstall { claude, codex, user } => {
            report_lines(provalot::init::uninstall(&root_from_cwd(), claude, codex, user))
        }
    }
}

fn root_from_cwd() -> std::path::PathBuf {
    let cwd = std::env::current_dir().expect("cwd");
    provalot::repo::find_root(&cwd)
}

fn report_lines(result: Result<Vec<String>, String>) {
    match result {
        Ok(lines) => {
            for l in lines {
                println!("{l}");
            }
        }
        Err(e) => {
            eprintln!("provalot: {e}");
            std::process::exit(1);
        }
    }
}
