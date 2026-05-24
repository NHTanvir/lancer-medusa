#![allow(dead_code, unused_imports)]

pub use crate::compilation::ContractDefinition;

use alloy_json_abi::Function;
use alloy_primitives::Address;

/// A deployed contract instance — definition + on-chain address.
#[derive(Debug, Clone)]
pub struct DeployedContract {
    pub definition: ContractDefinition,
    pub address: Address,
}

impl DeployedContract {
    /// Create a new `DeployedContract` pairing a definition with its address.
    pub fn new(definition: ContractDefinition, address: Address) -> Self {
        DeployedContract { definition, address }
    }

    /// Return all functions that are safe to call during fuzzing:
    /// excludes constructors, fallbacks, receives, view functions, and pure functions.
    pub fn fuzzable_functions(&self) -> Vec<Function> {
        self.definition
            .abi
            .functions()
            .filter(|f| {
                use alloy_json_abi::StateMutability;
                !matches!(
                    f.state_mutability,
                    StateMutability::View | StateMutability::Pure
                )
            })
            .cloned()
            .collect()
    }

    /// Return all functions whose name starts with any of `prefixes`.
    pub fn functions_with_prefix(&self, prefixes: &[&str]) -> Vec<Function> {
        self.definition
            .abi
            .functions()
            .filter(|f| prefixes.iter().any(|p| f.name.starts_with(p)))
            .cloned()
            .collect()
    }
}
