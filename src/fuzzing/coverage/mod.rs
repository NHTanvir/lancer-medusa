#![allow(dead_code)]

use std::collections::HashMap;

use dashmap::DashMap;
use alloy_primitives::Address;
use revm::{
    db::Database,
    interpreter::{opcode, CallInputs, CallOutcome, CreateInputs, CreateOutcome, Interpreter},
    EvmContext, Inspector,
};
use serde::{Deserialize, Serialize};

// ── ContractCoverageMap ──────────────────────────────────────────────────────

/// Per-contract coverage (marker → hit count).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ContractCoverageMap {
    pub markers: HashMap<u64, u64>,
}

impl ContractCoverageMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment the hit count for a marker.
    pub fn record(&mut self, marker: u64) {
        *self.markers.entry(marker).or_insert(0) += 1;
    }

    /// Returns `true` if `self` contains at least one marker absent from `other`.
    pub fn has_new_marker(&self, other: &ContractCoverageMap) -> bool {
        self.markers.keys().any(|k| !other.markers.contains_key(k))
    }

    /// Add all hit counts from `other` into `self`.
    pub fn merge_from(&mut self, other: &ContractCoverageMap) {
        for (marker, count) in &other.markers {
            *self.markers.entry(*marker).or_insert(0) += count;
        }
    }

    /// Total number of distinct markers seen.
    pub fn marker_count(&self) -> usize {
        self.markers.len()
    }
}

// ── CoverageMaps ─────────────────────────────────────────────────────────────

/// Global shared coverage across all workers.
#[derive(Debug, Default)]
pub struct CoverageMaps {
    inner: DashMap<Address, ContractCoverageMap>,
}

impl CoverageMaps {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update global coverage with a local map.
    ///
    /// Returns `true` if any new marker was discovered.
    pub fn update(&self, local: &LocalCoverage) -> bool {
        let mut found_new = false;
        for (addr, local_map) in &local.inner {
            let mut entry = self.inner.entry(*addr).or_default();
            if local_map.has_new_marker(&entry) {
                found_new = true;
            }
            entry.merge_from(local_map);
        }
        found_new
    }

    /// Total distinct marker count across all contracts.
    pub fn total_markers(&self) -> usize {
        self.inner.iter().map(|e| e.value().marker_count()).sum()
    }

    /// Cloned per-contract map for `addr`.
    pub fn get_contract(&self, addr: &Address) -> Option<ContractCoverageMap> {
        self.inner.get(addr).map(|e| e.value().clone())
    }

    /// Snapshot the current global state.
    pub fn snapshot(&self) -> HashMap<Address, ContractCoverageMap> {
        self.inner
            .iter()
            .map(|e| (*e.key(), e.value().clone()))
            .collect()
    }
}

// ── LocalCoverage ─────────────────────────────────────────────────────────────

/// Per-worker local coverage collected during one sequence execution.
#[derive(Debug, Default, Clone)]
pub struct LocalCoverage {
    inner: HashMap<Address, ContractCoverageMap>,
}

impl LocalCoverage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a marker hit for the given contract address.
    pub fn record(&mut self, contract: Address, marker: u64) {
        self.inner.entry(contract).or_default().record(marker);
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn get_contract(&self, addr: &Address) -> Option<&ContractCoverageMap> {
        self.inner.get(addr)
    }

    pub fn contracts(&self) -> impl Iterator<Item = (&Address, &ContractCoverageMap)> {
        self.inner.iter()
    }
}

// ── CoverageInspector ─────────────────────────────────────────────────────────

/// revm Inspector that instruments EVM execution and records Medusa-style
/// coverage markers into a `LocalCoverage`.
pub struct CoverageInspector {
    pub coverage: LocalCoverage,
    current_contract: Address,
    first_step: bool,
}

impl CoverageInspector {
    pub fn new() -> Self {
        Self {
            coverage: LocalCoverage::new(),
            current_contract: Address::ZERO,
            first_step: false,
        }
    }

    /// Consume the inspector and return the collected coverage.
    pub fn take_coverage(self) -> LocalCoverage {
        self.coverage
    }
}

impl Default for CoverageInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl<DB: Database> Inspector<DB> for CoverageInspector {
    /// Track the contract being entered.
    fn call(
        &mut self,
        _context: &mut EvmContext<DB>,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        self.current_contract = inputs.target_address;
        self.first_step = true;
        None
    }

    /// Core instrumentation: record coverage markers before each opcode.
    fn step(&mut self, interp: &mut Interpreter, _ctx: &mut EvmContext<DB>) {
        let pc = interp.program_counter();
        let op = interp.current_opcode();

        // Contract-entry marker on the very first step after a `call` callback.
        if self.first_step {
            self.first_step = false;
            let marker = 0xC000_0000u64 ^ pc as u64;
            self.coverage.record(self.current_contract, marker);
        }

        match op {
            opcode::JUMPI => {
                // Stack layout: TOS = dest, TOS-1 = condition.
                if let (Ok(dst), Ok(cond)) = (interp.stack.peek(0), interp.stack.peek(1)) {
                    if !cond.is_zero() {
                        let marker = (pc as u64) << 32
                            | (dst.saturating_to::<u32>() as u64);
                        self.coverage.record(self.current_contract, marker);
                    }
                }
            }
            opcode::REVERT => {
                let marker = (pc as u64) ^ 0x4000_0000u64;
                self.coverage.record(self.current_contract, marker);
            }
            opcode::RETURN => {
                let marker = (pc as u64) ^ 0x8000_0000u64;
                self.coverage.record(self.current_contract, marker);
            }
            _ => {}
        }
    }

    /// Track contract creation entry.
    fn create(
        &mut self,
        _context: &mut EvmContext<DB>,
        _inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.first_step = true;
        None
    }
}
