#![allow(dead_code, unused_imports)]

use std::collections::HashMap;
use std::sync::Mutex;

use alloy_primitives::{Bytes, I256};

use crate::fuzzing::calls::{CallSequence, SequenceExecutionResult};
use crate::fuzzing::contracts::DeployedContract;
use crate::fuzzing::test_case::{TestCaseProvider, TestCaseResult, TestStatus};

/// Test provider for optimization-based testing.
///
/// Tracks functions with `optimize_` prefix (or custom prefixes) that return
/// `int256`. The goal is to maximize the return value; every time a new
/// maximum is found a `Passed` result is emitted so the corpus can record it.
pub struct OptimizationTestProvider {
    pub prefixes: Vec<String>,
    best_values: Mutex<HashMap<String, I256>>,
}

impl OptimizationTestProvider {
    /// Create a provider with the default prefix `optimize_`.
    pub fn new() -> Self {
        OptimizationTestProvider {
            prefixes: vec!["optimize_".to_string()],
            best_values: Mutex::new(HashMap::new()),
        }
    }

    /// Return the best `int256` value seen so far for a given function name.
    pub fn best_value(&self, function_name: &str) -> Option<I256> {
        self.best_values
            .lock()
            .expect("best_values mutex poisoned")
            .get(function_name)
            .copied()
    }

    /// Parse a 32-byte ABI-encoded `int256` from `output`.
    fn parse_i256(output: &Bytes) -> Option<I256> {
        if output.len() != 32 {
            return None;
        }
        let arr: [u8; 32] = output[..32].try_into().ok()?;
        Some(I256::from_be_bytes(arr))
    }
}

impl Default for OptimizationTestProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCaseProvider for OptimizationTestProvider {
    fn name(&self) -> &str {
        "Optimization"
    }

    fn detect_tests(&self, contract: &DeployedContract) -> Vec<(String, String)> {
        let prefix_strs: Vec<&str> = self.prefixes.iter().map(|s| s.as_str()).collect();
        contract
            .functions_with_prefix(&prefix_strs)
            .into_iter()
            .map(|f| (f.name.clone(), contract.definition.name.clone()))
            .collect()
    }

    fn analyze(
        &self,
        sequence: &CallSequence,
        results: &SequenceExecutionResult,
        contract: &DeployedContract,
    ) -> Vec<TestCaseResult> {
        let prefix_strs: Vec<&str> = self.prefixes.iter().map(|s| s.as_str()).collect();
        let test_names: std::collections::HashSet<String> = contract
            .functions_with_prefix(&prefix_strs)
            .into_iter()
            .map(|f| f.name)
            .collect();

        let mut found = Vec::new();

        for result in &results.results {
            let fn_name = &result.call.function_name;
            if !test_names.contains(fn_name) {
                continue;
            }
            if !result.success {
                continue;
            }
            let value = match Self::parse_i256(&result.output) {
                Some(v) => v,
                None => continue,
            };

            // Check whether this beats the current best.
            let mut best_map = self
                .best_values
                .lock()
                .expect("best_values mutex poisoned");
            let is_better = match best_map.get(fn_name) {
                Some(&prev) => value > prev,
                None => true,
            };

            if is_better {
                best_map.insert(fn_name.clone(), value);
                // Drop the lock before building the result to avoid holding it
                // while cloning the (potentially large) sequence.
                drop(best_map);

                found.push(TestCaseResult {
                    status: TestStatus::Passed,
                    name: fn_name.clone(),
                    contract: contract.definition.name.clone(),
                    message: format!("Optimization value: {}", value),
                    failing_sequence: Some(sequence.clone()),
                });
            }
        }

        found
    }
}
