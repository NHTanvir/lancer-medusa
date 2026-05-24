#![allow(dead_code, unused_imports)]

/// ContractDefinition holds ABI + bytecode for a fuzz target.
/// Full implementation comes in a later task.
pub struct ContractDefinition {
    pub name: String,
    pub bytecode: Vec<u8>,
    pub deployed_bytecode: Vec<u8>,
}

impl ContractDefinition {
    pub fn new(name: impl Into<String>) -> Self {
        ContractDefinition {
            name: name.into(),
            bytecode: Vec::new(),
            deployed_bytecode: Vec::new(),
        }
    }
}
