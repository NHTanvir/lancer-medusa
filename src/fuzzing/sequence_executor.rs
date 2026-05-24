#![allow(dead_code)]

use crate::chain::TestChain;
use crate::fuzzing::calls::{CallExecutionResult, CallSequence, SequenceExecutionResult};

/// Executes call sequences against a `TestChain`.
pub struct SequenceExecutor<'a> {
    chain: &'a mut TestChain,
}

impl<'a> SequenceExecutor<'a> {
    pub fn new(chain: &'a mut TestChain) -> Self {
        SequenceExecutor { chain }
    }

    /// Execute all calls in `sequence` in order.
    ///
    /// Execution continues even when a call reverts — every call is attempted
    /// and its result is recorded.  If the chain itself returns an error (an
    /// EVM-level failure that cannot be attributed to a revert), that call is
    /// recorded as a failure and execution stops; `reverted_at` is set to that
    /// call index.
    pub fn execute(&mut self, sequence: &CallSequence) -> SequenceExecutionResult {
        let mut result = SequenceExecutionResult::default();

        for (idx, call) in sequence.calls.iter().enumerate() {
            let calldata = call.to_calldata();

            match self.chain.call(call.from, call.to, calldata, call.value) {
                Ok(chain_result) => {
                    result.results.push(CallExecutionResult {
                        call: call.clone(),
                        success: chain_result.success,
                        output: chain_result.output,
                        gas_used: chain_result.gas_used,
                        revert_reason: chain_result.revert_reason,
                    });
                }
                Err(_) => {
                    // EVM-level error (e.g. environment / infrastructure fault).
                    // Record a synthetic failure and stop the sequence.
                    result.results.push(CallExecutionResult {
                        call: call.clone(),
                        success: false,
                        output: alloy_primitives::Bytes::new(),
                        gas_used: 0,
                        revert_reason: None,
                    });
                    result.reverted_at = Some(idx);
                    break;
                }
            }
        }

        result
    }
}
