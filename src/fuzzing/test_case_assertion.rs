#![allow(dead_code, unused_imports)]

use alloy_primitives::Bytes;
use serde::{Deserialize, Serialize};

use crate::fuzzing::calls::{CallSequence, SequenceExecutionResult};
use crate::fuzzing::contracts::DeployedContract;
use crate::fuzzing::test_case::{TestCaseProvider, TestCaseResult, TestStatus};

/// Solidity panic selector: `Panic(uint256)` = keccak256("Panic(uint256)")[0..4]
const PANIC_SELECTOR: [u8; 4] = [0x4e, 0x48, 0x7b, 0x71];

/// Configuration controlling which Solidity panic codes trigger a failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanicCodeConfig {
    /// Assert failure (0x01)
    pub assert_fail: bool,
    /// Arithmetic overflow/underflow (0x11)
    pub arithmetic_overflow: bool,
    /// Division by zero (0x12)
    pub div_by_zero: bool,
    /// Invalid enum value (0x21)
    pub invalid_enum: bool,
    /// Invalid storage access (0x22)
    pub invalid_storage: bool,
    /// Array pop on empty array (0x31)
    pub array_underflow: bool,
    /// Array index out of bounds (0x32)
    pub array_overflow: bool,
    /// Out of memory (0x41)
    pub out_of_memory: bool,
    /// Call to zero function pointer (0x51)
    pub zero_fn_ptr: bool,
}

impl Default for PanicCodeConfig {
    fn default() -> Self {
        PanicCodeConfig {
            assert_fail: true,
            arithmetic_overflow: true,
            div_by_zero: true,
            invalid_enum: true,
            invalid_storage: true,
            array_underflow: true,
            array_overflow: true,
            out_of_memory: true,
            zero_fn_ptr: true,
        }
    }
}

/// Test provider for assertion-based testing.
///
/// Detects EVM panic codes in revert data. The Solidity panic format is:
/// 4-byte selector `0x4e487b71` followed by a 32-byte ABI-encoded `uint256`
/// panic code.
pub struct AssertionTestProvider {
    pub config: PanicCodeConfig,
}

impl AssertionTestProvider {
    /// Create a provider with all panic codes enabled.
    pub fn new() -> Self {
        AssertionTestProvider {
            config: PanicCodeConfig::default(),
        }
    }

    /// Parse the panic code from revert reason bytes.
    ///
    /// Format: `[selector 4 bytes][uint256 panic code 32 bytes]`.
    /// Returns `None` if the bytes don't match the panic format.
    fn parse_panic_code(revert_reason: &Bytes) -> Option<u64> {
        if revert_reason.len() < 36 {
            return None;
        }
        if revert_reason[0..4] != PANIC_SELECTOR {
            return None;
        }
        // The panic code is ABI-encoded as a uint256 in bytes 4..36 (big-endian).
        // All known Solidity panic codes fit in one byte, so we read the last byte.
        let code = revert_reason[35] as u64;
        Some(code)
    }

    /// Check whether a given panic code is enabled in the current config.
    fn is_panic_code_enabled(&self, code: u64) -> bool {
        match code {
            0x01 => self.config.assert_fail,
            0x11 => self.config.arithmetic_overflow,
            0x12 => self.config.div_by_zero,
            0x21 => self.config.invalid_enum,
            0x22 => self.config.invalid_storage,
            0x31 => self.config.array_underflow,
            0x32 => self.config.array_overflow,
            0x41 => self.config.out_of_memory,
            0x51 => self.config.zero_fn_ptr,
            _ => false,
        }
    }

    /// Human-readable description for a panic code.
    fn panic_code_name(code: u64) -> &'static str {
        match code {
            0x01 => "assert failure",
            0x11 => "arithmetic overflow",
            0x12 => "division by zero",
            0x21 => "invalid enum value",
            0x22 => "invalid storage access",
            0x31 => "array pop on empty array",
            0x32 => "array index out of bounds",
            0x41 => "out of memory",
            0x51 => "call to zero function pointer",
            _ => "unknown panic",
        }
    }
}

impl Default for AssertionTestProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TestCaseProvider for AssertionTestProvider {
    fn name(&self) -> &str {
        "Assertion"
    }

    fn detect_tests(&self, contract: &DeployedContract) -> Vec<(String, String)> {
        // Any non-view, non-pure function can trigger an assert/panic.
        contract
            .fuzzable_functions()
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
        let mut found = Vec::new();

        for result in &results.results {
            if result.success {
                continue;
            }
            let revert_reason = match &result.revert_reason {
                Some(r) => r,
                None => continue,
            };
            let code = match Self::parse_panic_code(revert_reason) {
                Some(c) => c,
                None => continue,
            };
            if !self.is_panic_code_enabled(code) {
                continue;
            }
            found.push(TestCaseResult {
                status: TestStatus::Failed,
                name: result.call.function_name.clone(),
                contract: contract.definition.name.clone(),
                message: format!(
                    "Panic in '{}': {} (code 0x{:02x})",
                    result.call.function_name,
                    Self::panic_code_name(code),
                    code
                ),
                failing_sequence: Some(sequence.clone()),
            });
        }

        found
    }
}
