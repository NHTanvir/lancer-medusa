#![allow(dead_code, unused_imports)]

use serde::{Deserialize, Serialize};

/// Global fuzzing configuration — shared across all fuzzer subsystems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzingConfig {
    /// Maximum number of call sequences to generate.
    pub max_sequence_length: usize,
    /// Number of calls per generated sequence.
    pub call_sequence_length: usize,
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
    /// Function name prefixes used for property-test detection (e.g. ["fuzz_", "test_"]).
    pub property_prefixes: Vec<String>,
}

impl Default for FuzzingConfig {
    fn default() -> Self {
        FuzzingConfig {
            max_sequence_length: 10_000,
            call_sequence_length: 10,
            workers: 1,
            seed: None,
            target_contracts: Vec::new(),
            coverage_guided: true,
            corpus_dir: None,
            property_prefixes: vec!["fuzz_".to_string(), "echidna_".to_string()],
        }
    }
}
