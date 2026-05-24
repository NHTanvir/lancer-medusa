#![allow(dead_code, unused_imports)]

/// ValueSet holds a pool of interesting values used during mutation.
/// Full implementation comes in a later task.
pub struct ValueSet {
    pub integers: Vec<u64>,
    pub addresses: Vec<[u8; 20]>,
    pub byte_arrays: Vec<Vec<u8>>,
}

impl ValueSet {
    pub fn new() -> Self {
        ValueSet {
            integers: Vec::new(),
            addresses: Vec::new(),
            byte_arrays: Vec::new(),
        }
    }
}

impl Default for ValueSet {
    fn default() -> Self {
        Self::new()
    }
}
