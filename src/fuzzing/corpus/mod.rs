#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::fuzzing::calls::CallSequence;

// ── CorpusEvent ───────────────────────────────────────────────────────────────

/// Events emitted by the corpus when interesting sequences are added.
#[derive(Debug, Clone)]
pub enum CorpusEvent {
    NewSequence { index: usize, sequence: CallSequence },
}

// ── Corpus ────────────────────────────────────────────────────────────────────

/// Stores interesting call sequences discovered during fuzzing.
///
/// Optionally disk-backed: when a directory is provided every new sequence is
/// serialised to `<dir>/corpus_NNNN.json`. Otherwise sequences live only in
/// memory.
pub struct Corpus {
    /// On-disk directory (None = memory-only).
    dir: Option<PathBuf>,
    /// In-memory sequence store.
    sequences: Vec<CallSequence>,
    /// Broadcast channel for corpus events.
    pub event_tx: broadcast::Sender<CorpusEvent>,
}

impl Corpus {
    // ── constructors ─────────────────────────────────────────────────────────

    /// Create a memory-only corpus (no disk persistence).
    pub fn new_in_memory() -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Corpus {
            dir: None,
            sequences: Vec::new(),
            event_tx,
        }
    }

    /// Create a disk-backed corpus at the given directory.
    ///
    /// The directory is created if it does not already exist. Any `*.json`
    /// files already present are loaded as the initial corpus.
    pub fn new(dir: &Path) -> anyhow::Result<Self> {
        fs::create_dir_all(dir)?;
        let sequences = Self::load_from_dir(dir)?;
        let (event_tx, _) = broadcast::channel(64);
        Ok(Corpus {
            dir: Some(dir.to_path_buf()),
            sequences,
            event_tx,
        })
    }

    // ── disk helpers ─────────────────────────────────────────────────────────

    /// Scan `dir` for `*.json` files and deserialise each as a [`CallSequence`].
    pub fn load_from_dir(dir: &Path) -> anyhow::Result<Vec<CallSequence>> {
        let mut sequences = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let data = fs::read_to_string(&path)?;
                match serde_json::from_str::<CallSequence>(&data) {
                    Ok(seq) => sequences.push(seq),
                    Err(e) => {
                        tracing::warn!("Skipping malformed corpus file {}: {}", path.display(), e);
                    }
                }
            }
        }
        Ok(sequences)
    }

    /// Serialise a single sequence to `<dir>/corpus_NNNN.json`.
    fn save_to_disk(&self, index: usize, seq: &CallSequence) -> anyhow::Result<()> {
        if let Some(dir) = &self.dir {
            let filename = format!("corpus_{:04}.json", index);
            let path = dir.join(filename);
            let json = serde_json::to_string_pretty(seq)?;
            fs::write(&path, json)?;
        }
        Ok(())
    }

    // ── mutation ──────────────────────────────────────────────────────────────

    /// Add a new sequence to the corpus.
    ///
    /// If the corpus is disk-backed the sequence is immediately written to
    /// disk. A [`CorpusEvent::NewSequence`] is broadcast regardless.
    pub fn add(&mut self, seq: CallSequence) -> anyhow::Result<()> {
        let index = self.sequences.len();
        self.sequences.push(seq.clone());
        self.save_to_disk(index, &seq)?;
        // Ignore send errors — it just means there are no active receivers.
        let _ = self.event_tx.send(CorpusEvent::NewSequence { index, sequence: seq });
        Ok(())
    }

    // ── queries ───────────────────────────────────────────────────────────────

    /// All sequences currently held in the corpus.
    pub fn sequences(&self) -> &[CallSequence] {
        &self.sequences
    }

    /// Number of sequences in the corpus.
    pub fn len(&self) -> usize {
        self.sequences.len()
    }

    /// Returns `true` when the corpus contains no sequences.
    pub fn is_empty(&self) -> bool {
        self.sequences.is_empty()
    }

    /// Subscribe to corpus events via a tokio broadcast receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<CorpusEvent> {
        self.event_tx.subscribe()
    }

    // ── pruning ───────────────────────────────────────────────────────────────

    /// Remove sequences that do not contribute new coverage markers.
    ///
    /// **Note:** `CallSequence` does not currently carry per-sequence coverage
    /// metadata. Full coverage-aware pruning requires that information, which
    /// will be wired in a later task. For now this is a no-op stub.
    ///
    /// Returns the number of sequences pruned (currently always 0).
    pub fn prune(&mut self) -> usize {
        tracing::info!("prune not yet implemented with coverage data");
        0
    }
}

impl Default for Corpus {
    fn default() -> Self {
        Self::new_in_memory()
    }
}
