#![allow(dead_code, unused_imports)]

pub mod calls;
pub mod config;
pub mod contracts;
pub mod corpus;
pub mod coverage;
pub mod valuegeneration;

use crate::fuzzing::config::FuzzingConfig;

/// Fuzzer orchestrates the full fuzzing campaign.
/// Full implementation comes in a later task.
pub struct Fuzzer {
    pub config: FuzzingConfig,
}

impl Fuzzer {
    pub fn new(config: FuzzingConfig) -> Self {
        Fuzzer { config }
    }
}
