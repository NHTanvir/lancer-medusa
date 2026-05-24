#![allow(dead_code, unused_imports)]

pub mod calls;
pub mod config;
pub mod contracts;
pub mod corpus;
pub mod coverage;
pub mod feedback;
pub mod sequence_executor;
pub mod sequence_generator;
pub mod test_case;
pub mod test_case_assertion;
pub mod test_case_optimization;
pub mod test_case_property;
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
