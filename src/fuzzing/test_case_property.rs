#![allow(dead_code, unused_imports)]

use alloy_primitives::Bytes;

use crate::fuzzing::calls::{CallSequence, SequenceExecutionResult};
use crate::fuzzing::contracts::DeployedContract;
use crate::fuzzing::test_case::{TestCaseProvider, TestCaseResult, TestStatus};

/// Test provider for property-based testing.
///
/// Detects functions with `fuzz_`, `property_`, or `echidna_` prefixes that
/// return `bool`. A failing property is one that returns `false` (i.e.,
/// `success == true` but output is 32 zero bytes).
pub struct PropertyTestProvider {
    pub prefixes: Vec<String>,
}

impl PropertyTestProvider {
    /// Create a provider with the default prefixes: `fuzz_`, `property_`, `echidna_`.
    pub fn new() -> Self {
        Self::with_prefixes(vec![
            "fuzz_".to_string(),
            "property_".to_string(),
            "echidna_".to_string(),
        ])
    }

    /// Create a provider with custom prefixes.
    pub fn with_prefixes(prefixes: Vec<String>) -> Self {
        PropertyTestProvider { prefixes }
    }
}

impl Default for PropertyTestProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCaseProvider for PropertyTestProvider {
    fn name(&self) -> &str {
        "Property"
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
        let false_output = Bytes::from(vec![0u8; 32]);
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
            // A property fails when the call succeeded but returned bool false
            // (ABI-encoded false == 32 zero bytes).
            if result.success && result.output == false_output {
                found.push(TestCaseResult {
                    status: TestStatus::Failed,
                    name: fn_name.clone(),
                    contract: contract.definition.name.clone(),
                    message: format!(
                        "Property '{}' returned false",
                        fn_name
                    ),
                    failing_sequence: Some(sequence.clone()),
                });
            }
        }

        found
    }
}
