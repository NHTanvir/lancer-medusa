#![allow(dead_code, unused_imports)]

/// Compilation result placeholder.
/// Full implementation (Foundry/Hardhat artifact parsing) comes in a later task.
pub struct CompilationResult;

impl CompilationResult {
    pub fn new() -> Self {
        CompilationResult
    }
}

impl Default for CompilationResult {
    fn default() -> Self {
        Self::new()
    }
}
