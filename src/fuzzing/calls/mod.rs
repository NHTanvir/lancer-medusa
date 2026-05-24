#![allow(dead_code, unused_imports)]

use alloy_primitives::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};

use crate::fuzzing::valuegeneration::AbiValue;

// ---------------------------------------------------------------------------
// FuzzCall
// ---------------------------------------------------------------------------

/// A single call to be made during a fuzzing iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzCall {
    pub from: Address,
    pub to: Address,
    /// The 4-byte function selector.
    pub selector: [u8; 4],
    /// Decoded arguments (kept for mutation in subsequent iterations).
    pub args: Vec<AbiValue>,
    pub value: U256,
    pub function_name: String,
}

impl FuzzCall {
    pub fn new(
        from: Address,
        to: Address,
        selector: [u8; 4],
        args: Vec<AbiValue>,
        function_name: &str,
    ) -> Self {
        FuzzCall {
            from,
            to,
            selector,
            args,
            value: U256::ZERO,
            function_name: function_name.to_string(),
        }
    }

    /// Encode this call into ABI calldata: selector ++ abi_encode(each arg).
    pub fn to_calldata(&self) -> Bytes {
        let mut out = Vec::with_capacity(4 + self.args.len() * 32);
        out.extend_from_slice(&self.selector);
        for arg in &self.args {
            out.extend_from_slice(&arg.abi_encode());
        }
        Bytes::from(out)
    }
}

// ---------------------------------------------------------------------------
// CallSequence
// ---------------------------------------------------------------------------

/// An ordered sequence of calls representing one fuzzing iteration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CallSequence {
    pub calls: Vec<FuzzCall>,
}

impl CallSequence {
    pub fn new() -> Self {
        CallSequence { calls: Vec::new() }
    }

    pub fn push(&mut self, call: FuzzCall) {
        self.calls.push(call);
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}

// ---------------------------------------------------------------------------
// CallExecutionResult / SequenceExecutionResult
// ---------------------------------------------------------------------------

/// The result of executing one call within a sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallExecutionResult {
    pub call: FuzzCall,
    pub success: bool,
    pub output: Bytes,
    pub gas_used: u64,
    pub revert_reason: Option<Bytes>,
}

/// The aggregate result of executing a full `CallSequence`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SequenceExecutionResult {
    pub results: Vec<CallExecutionResult>,
    /// Index of the first call that caused a hard-revert stopping execution,
    /// or `None` if all calls were attempted.
    pub reverted_at: Option<usize>,
}
