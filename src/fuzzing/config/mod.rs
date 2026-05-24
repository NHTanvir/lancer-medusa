#![allow(dead_code, unused_imports)]

use serde::{Deserialize, Serialize};

/// Global fuzzing configuration — shared across all fuzzer subsystems.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FuzzingConfig {
    /// Maximum number of call sequences to generate.
    pub max_sequence_length: usize,
    /// Number of parallel worker threads.
    pub workers: usize,
    /// Seed for deterministic RNG (None = random).
    pub seed: Option<u64>,
    /// Target contract names to fuzz.
    pub target_contracts: Vec<String>,
    /// Enable coverage-guided fuzzing.
    pub coverage_guided: bool,
    /// Corpus directory path.
    pub corpus_dir: Option<String>,
}
