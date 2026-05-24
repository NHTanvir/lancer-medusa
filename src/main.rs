//! lancer-medusa
//! Rust clone of medusa. See plan.md (local, gitignored) for the spec.

#![allow(dead_code, unused_imports)]

use clap::{Parser, Subcommand};

pub mod chain;
pub mod compilation;
pub mod fuzzing;

#[derive(Parser, Debug)]
#[command(
    name = "lancer-medusa",
    version,
    about = "Rust clone of medusa — smart contract fuzzer (Lancer toolkit)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the fuzzer against a compiled project.
    Fuzz {
        /// Path to the project directory.
        #[arg(short, long, default_value = ".")]
        project: String,

        /// Number of parallel workers.
        #[arg(short, long, default_value_t = 4)]
        workers: usize,
    },

    /// Initialise a new lancer-medusa config in the current directory.
    Init {
        /// Target framework (foundry | hardhat).
        #[arg(short, long)]
        framework: Option<String>,
    },

    /// Manage the fuzzing corpus.
    Corpus {
        #[command(subcommand)]
        action: CorpusAction,
    },
}

#[derive(Subcommand, Debug)]
enum CorpusAction {
    /// Remove corpus entries that no longer increase coverage.
    Prune {
        /// Path to the corpus directory.
        #[arg(short, long, default_value = "corpus")]
        dir: String,
    },
    /// Report corpus statistics and integrity.
    Health {
        /// Path to the corpus directory.
        #[arg(short, long, default_value = "corpus")]
        dir: String,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Fuzz { project, workers } => {
            println!("fuzz: project={project} workers={workers} (placeholder)");
        }
        Commands::Init { framework } => {
            let fw = framework.as_deref().unwrap_or("foundry");
            println!("init: framework={fw} (placeholder)");
        }
        Commands::Corpus {
            action: CorpusAction::Prune { dir },
        } => {
            println!("corpus prune: dir={dir} (placeholder)");
        }
        Commands::Corpus {
            action: CorpusAction::Health { dir },
        } => {
            println!("corpus health: dir={dir} (placeholder)");
        }
    }

    Ok(())
}
