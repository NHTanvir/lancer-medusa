//! lancer-medusa
//! Rust clone of medusa. See plan.md (local, gitignored) for the spec.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "lancer-medusa", version, about = "Rust clone of medusa")]
struct Cli {}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let _cli = Cli::parse();
    println!("lancer-medusa - scaffold ready. Implement per plan.md.");
    Ok(())
}
