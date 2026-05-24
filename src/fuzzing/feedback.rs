#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::fuzzing::calls::CallSequence;
use crate::fuzzing::corpus::Corpus;
use crate::fuzzing::coverage::{CoverageMaps, LocalCoverage};

// ── FeedbackLoop ─────────────────────────────────────────────────────────────

/// Connects coverage tracking, the corpus, and the sequence generator.
///
/// After each sequence execution the caller passes the resulting
/// [`LocalCoverage`] to [`FeedbackLoop::process`]. If that coverage contains
/// markers not yet seen globally, the sequence is added to the corpus and the
/// method returns `true`.
pub struct FeedbackLoop {
    coverage: Arc<CoverageMaps>,
    corpus: Arc<Mutex<Corpus>>,
}

impl FeedbackLoop {
    /// Create a new feedback loop wired to the given shared coverage and corpus.
    pub fn new(coverage: Arc<CoverageMaps>, corpus: Arc<Mutex<Corpus>>) -> Self {
        FeedbackLoop { coverage, corpus }
    }

    /// Process coverage from one sequence execution.
    ///
    /// Updates global coverage with `local_coverage`. If new markers are
    /// discovered the sequence is added to the corpus and `true` is returned.
    /// Otherwise `false` is returned and the sequence is discarded.
    pub async fn process(
        &self,
        sequence: CallSequence,
        local_coverage: LocalCoverage,
    ) -> anyhow::Result<bool> {
        let is_new = self.coverage.update(&local_coverage);
        if is_new {
            let mut corpus = self.corpus.lock().await;
            corpus.add(sequence)?;
        }
        Ok(is_new)
    }
}
