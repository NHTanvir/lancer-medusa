//! lancer-medusa
//! Rust clone of medusa. See plan.md (local, gitignored) for the spec.

#![allow(dead_code, unused_imports)]

use std::path::Path;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};

mod report;

pub mod chain;
pub mod compilation;
pub mod fuzzing;

use crate::fuzzing::config::FuzzingConfig;
use crate::fuzzing::corpus::Corpus;

// ── CLI types ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "lancer-medusa",
    version,
    about = "Parallelized coverage-guided smart contract fuzzer"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Log level: error, warn, info, debug, trace
    #[arg(long, default_value = "info", global = true)]
    log_level: String,

    /// Disable ANSI colors in output
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a fuzz campaign
    Fuzz(FuzzArgs),
    /// Scaffold a medusa.json config file
    Init(InitArgs),
    /// Corpus management
    Corpus {
        #[command(subcommand)]
        cmd: CorpusCommands,
    },
}

#[derive(Subcommand)]
enum CorpusCommands {
    /// Remove redundant sequences
    Prune(CorpusPruneArgs),
    /// Show corpus health
    Health(CorpusHealthArgs),
}

// ── FuzzArgs ──────────────────────────────────────────────────────────────────

#[derive(Args)]
struct FuzzArgs {
    /// Path to medusa.json config file
    #[arg(long)]
    config: Option<String>,

    /// Number of parallel workers
    #[arg(long)]
    workers: Option<usize>,

    /// Campaign timeout in seconds
    #[arg(long)]
    timeout: Option<u64>,

    /// Maximum transaction count
    #[arg(long)]
    test_limit: Option<u64>,

    /// Maximum calls per sequence (0 = use default)
    #[arg(long, default_value = "0")]
    seq_len: usize,

    /// Contract names to fuzz (may be repeated)
    #[arg(long)]
    target_contracts: Vec<String>,

    /// Corpus directory
    #[arg(long)]
    corpus_dir: Option<String>,

    /// Sender addresses in hex (may be repeated)
    #[arg(long)]
    senders: Vec<String>,

    /// Deployer address in hex
    #[arg(long)]
    deployer: Option<String>,

    /// Stop on first failure
    #[arg(long)]
    fail_fast: bool,

    /// Exploration mode — gather coverage without property testing
    #[arg(long)]
    explore: bool,

    /// Foundry/Hardhat compilation target
    #[arg(long)]
    compilation_target: Option<String>,

    /// Verbosity level (repeat for more: -v, -vv, -vvv)
    #[arg(short, long, default_value = "0")]
    verbosity: u8,
}

// ── InitArgs ──────────────────────────────────────────────────────────────────

#[derive(Args)]
struct InitArgs {
    /// Project type: foundry, hardhat, truffle
    #[arg(default_value = "foundry")]
    framework: String,
}

// ── CorpusPruneArgs / CorpusHealthArgs ────────────────────────────────────────

#[derive(Args)]
struct CorpusPruneArgs {
    /// Path to corpus directory
    #[arg(long, default_value = "corpus")]
    dir: String,
}

#[derive(Args)]
struct CorpusHealthArgs {
    /// Path to corpus directory
    #[arg(long, default_value = "corpus")]
    dir: String,
}

// ── entry point ───────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    setup_logging(&cli.log_level)?;

    match cli.command {
        Commands::Fuzz(args) => cmd_fuzz(args),
        Commands::Init(args) => cmd_init(args),
        Commands::Corpus { cmd } => match cmd {
            CorpusCommands::Prune(args) => cmd_corpus_prune(args),
            CorpusCommands::Health(args) => cmd_corpus_health(args),
        },
    }
}

// ── logging setup ─────────────────────────────────────────────────────────────

fn setup_logging(level: &str) -> anyhow::Result<()> {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_new(level)
        .unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
    Ok(())
}

// ── cmd_fuzz ──────────────────────────────────────────────────────────────────

fn cmd_fuzz(args: FuzzArgs) -> anyhow::Result<()> {
    // Build FuzzingConfig from CLI arguments.
    let mut config = FuzzingConfig::default();

    if let Some(w) = args.workers {
        config.workers = Some(w);
    }
    if let Some(t) = args.timeout {
        config.timeout_secs = Some(t);
    }
    if let Some(l) = args.test_limit {
        config.test_limit = Some(l);
    }
    if args.seq_len > 0 {
        config.call_sequence_length = args.seq_len;
    }
    if !args.target_contracts.is_empty() {
        config.target_contracts = args.target_contracts.clone();
    }
    if let Some(ref d) = args.corpus_dir {
        config.corpus_dir = Some(d.clone());
    }
    if args.fail_fast {
        config.stop_on_failure = true;
    }

    // Print campaign summary.
    println!("lancer-medusa: starting fuzzer");
    println!("  workers       : {:?}", config.workers);
    println!("  timeout_secs  : {:?}", config.timeout_secs);
    println!("  test_limit    : {:?}", config.test_limit);
    println!("  seq_len       : {}", config.call_sequence_length);
    println!("  stop_on_fail  : {}", config.stop_on_failure);
    println!("  explore_mode  : {}", args.explore);

    if let Some(ref tgt) = args.compilation_target {
        println!("  compile target: {}", tgt);
    }
    if !args.senders.is_empty() {
        println!("  senders       : {}", args.senders.join(", "));
    }
    if let Some(ref d) = args.deployer {
        println!("  deployer      : {}", d);
    }
    if let Some(ref cfg_path) = args.config {
        println!("  config file   : {}", cfg_path);
    }

    if let Some(ref dir) = config.corpus_dir {
        println!("  corpus dir    : {}", dir);
    } else {
        println!("  corpus dir    : (in-memory only)");
    }

    println!();
    println!("Note: compile and deploy your contracts first, then wire into Fuzzer::new()");

    Ok(())
}

// ── cmd_init ──────────────────────────────────────────────────────────────────

fn cmd_init(args: InitArgs) -> anyhow::Result<()> {
    let template = medusa_json_template(&args.framework);
    let dest = Path::new("medusa.json");
    if dest.exists() {
        anyhow::bail!("medusa.json already exists in the current directory");
    }
    std::fs::write(dest, &template)
        .context("failed to write medusa.json")?;
    println!("lancer-medusa: created medusa.json for framework '{}'", args.framework);
    Ok(())
}

fn medusa_json_template(framework: &str) -> String {
    let platform = match framework {
        "hardhat" => "hardhat",
        "truffle" => "truffle",
        _ => "crytic-compile",   // foundry default
    };
    format!(
        r#"{{
  "fuzzing": {{
    "workers": 0,
    "timeout": 0,
    "testLimit": 0,
    "callSequenceLength": 100,
    "shrinkLimit": 500,
    "corpusDirectory": "corpus"
  }},
  "testing": {{
    "stopOnFailedTest": false,
    "testAllContracts": true,
    "propertyTesting": {{
      "enabled": true,
      "testPrefixes": ["fuzz_", "property_", "echidna_"]
    }},
    "assertionTesting": {{
      "enabled": true
    }},
    "optimizationTesting": {{
      "enabled": false,
      "testPrefixes": ["optimize_"]
    }}
  }},
  "compilation": {{
    "target": "",
    "platform": "{platform}"
  }}
}}
"#
    )
}

// ── cmd_corpus_prune ──────────────────────────────────────────────────────────

fn cmd_corpus_prune(args: CorpusPruneArgs) -> anyhow::Result<()> {
    let dir = Path::new(&args.dir);
    let before;
    let pruned;
    {
        let mut corpus = Corpus::new(dir)
            .with_context(|| format!("failed to load corpus from '{}'", args.dir))?;
        before = corpus.len();
        pruned = corpus.prune();
    }
    println!(
        "lancer-medusa corpus prune: dir='{}' sequences_before={} pruned={} sequences_after={}",
        args.dir,
        before,
        pruned,
        before - pruned
    );
    Ok(())
}

// ── cmd_corpus_health ─────────────────────────────────────────────────────────

fn cmd_corpus_health(args: CorpusHealthArgs) -> anyhow::Result<()> {
    let dir = Path::new(&args.dir);
    let corpus = Corpus::new(dir)
        .with_context(|| format!("failed to load corpus from '{}'", args.dir))?;
    println!(
        "lancer-medusa corpus health: dir='{}' sequences={}",
        args.dir,
        corpus.len()
    );
    Ok(())
}
