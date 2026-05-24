#![allow(dead_code, unused_imports)]

use serde::{Deserialize, Serialize};

/// Global fuzzing configuration — shared across all fuzzer subsystems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzingConfig {
    /// Maximum number of call sequences to generate.
    pub max_sequence_length: usize,
    /// Number of calls per generated sequence.
    pub call_sequence_length: usize,
    /// Number of parallel worker threads (None = use rayon default).
    pub workers: Option<usize>,
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
    /// Stop the campaign after this many seconds (None = run forever).
    pub timeout_secs: Option<u64>,
    /// Stop after this many sequences have been executed (None = no limit).
    pub test_limit: Option<u64>,
    /// Maximum shrinking iterations when reducing a failing sequence.
    pub shrink_limit: usize,
    /// Stop the entire campaign as soon as the first failure is found.
    pub stop_on_failure: bool,
}

impl Default for FuzzingConfig {
    fn default() -> Self {
        FuzzingConfig {
            max_sequence_length: 10_000,
            call_sequence_length: 10,
            workers: None,
            seed: None,
            target_contracts: Vec::new(),
            coverage_guided: true,
            corpus_dir: None,
            property_prefixes: vec!["fuzz_".to_string(), "echidna_".to_string()],
            timeout_secs: None,
            test_limit: None,
            shrink_limit: 500,
            stop_on_failure: false,
        }
    }
}
