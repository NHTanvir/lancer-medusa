#![allow(dead_code, unused_imports)]

use alloy_primitives::Bytes;
use serde::{Deserialize, Serialize};

use crate::fuzzing::calls::{CallSequence, SequenceExecutionResult};

/// Status of a test case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TestStatus {
    /// Not yet found to fail or pass.
    Unresolved,
    /// Failure found.
    Failed,
    /// Confirmed passing (optimization: reached limit).
    Passed,
}

/// A test case result that's been found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseResult {
    pub status: TestStatus,
    pub name: String,
    pub contract: String,
    pub message: String,
    pub failing_sequence: Option<CallSequence>,
}

/// Trait that test providers implement.
pub trait TestCaseProvider: Send + Sync {
    /// Name of this test mode.
    fn name(&self) -> &str;

    /// Detect test functions in a deployed contract.
    /// Returns list of (function_name, contract_name) pairs.
    fn detect_tests(
        &self,
        contract: &crate::fuzzing::contracts::DeployedContract,
    ) -> Vec<(String, String)>;

    /// Analyze execution results for failures.
    /// Returns any new TestCaseResult found (status != Unresolved).
    fn analyze(
        &self,
        sequence: &CallSequence,
        results: &SequenceExecutionResult,
        contract: &crate::fuzzing::contracts::DeployedContract,
    ) -> Vec<TestCaseResult>;
}
