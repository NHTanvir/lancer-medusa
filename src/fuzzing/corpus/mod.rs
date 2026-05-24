#![allow(dead_code, unused_imports)]

use crate::fuzzing::calls::CallSequence;

/// Corpus stores interesting call sequences discovered during fuzzing.
/// Full implementation comes in a later task.
pub struct Corpus {
    pub entries: Vec<CallSequence>,
}

impl Corpus {
    pub fn new() -> Self {
        Corpus {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Corpus {
    fn default() -> Self {
        Self::new()
    }
}
