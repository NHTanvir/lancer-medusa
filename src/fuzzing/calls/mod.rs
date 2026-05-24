#![allow(dead_code, unused_imports)]

/// Represents a sequence of ABI-encoded calls to fuzz with.
/// Full implementation comes in a later task.
pub struct CallSequence {
    pub calls: Vec<FuzzCall>,
}

pub struct FuzzCall {
    pub contract: String,
    pub selector: [u8; 4],
    pub calldata: Vec<u8>,
}

impl CallSequence {
    pub fn new() -> Self {
        CallSequence { calls: Vec::new() }
    }
}

impl Default for CallSequence {
    fn default() -> Self {
        Self::new()
    }
}
