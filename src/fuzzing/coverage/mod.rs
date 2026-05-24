#![allow(dead_code, unused_imports)]

use dashmap::DashMap;

/// CoverageMaps tracks per-contract PC coverage across fuzzing runs.
/// Full implementation comes in a later task.
pub struct CoverageMaps {
    /// Maps contract address (hex) -> set of covered program counters.
    pub maps: DashMap<String, Vec<u64>>,
}

impl CoverageMaps {
    pub fn new() -> Self {
        CoverageMaps {
            maps: DashMap::new(),
        }
    }

    pub fn total_covered(&self) -> usize {
        self.maps.iter().map(|e| e.value().len()).sum()
    }
}

impl Default for CoverageMaps {
    fn default() -> Self {
        Self::new()
    }
}
