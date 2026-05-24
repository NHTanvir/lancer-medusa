#![allow(dead_code, unused_imports)]

/// TestChain provides an in-process EVM chain for fuzzing.
/// Full implementation comes in a later task.
pub struct TestChain;

impl TestChain {
    pub fn new() -> Self {
        TestChain
    }
}

impl Default for TestChain {
    fn default() -> Self {
        Self::new()
    }
}
