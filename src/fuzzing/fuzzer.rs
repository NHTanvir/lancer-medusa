#![allow(dead_code, unused_imports)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rayon::prelude::*;

use crate::chain::TestChain;
use crate::fuzzing::calls::CallSequence;
use crate::fuzzing::config::FuzzingConfig;
use crate::fuzzing::contracts::DeployedContract;
use crate::fuzzing::corpus::Corpus;
use crate::fuzzing::coverage::{CoverageMaps, LocalCoverage};
use crate::fuzzing::sequence_executor::SequenceExecutor;
use crate::fuzzing::sequence_generator::SequenceGenerator;
use crate::fuzzing::shrinking::Shrinker;
use crate::fuzzing::test_case::{TestCaseProvider, TestCaseResult, TestStatus};

// ── Fuzzer ────────────────────────────────────────────────────────────────────

/// Top-level fuzzing campaign orchestrator.
///
/// The fuzzer owns the configuration, the set of deployed contracts, shared
/// coverage maps, a corpus, and a list of test-case providers.  Calling
/// [`Fuzzer::run`] launches a parallel worker pool (via rayon) and blocks
/// until one of the stop conditions is met.
pub struct Fuzzer {
    pub config: FuzzingConfig,
    pub deployed: Vec<DeployedContract>,
    pub coverage: Arc<CoverageMaps>,
    /// `std::sync::Mutex` so rayon workers can lock without async.
    pub corpus: Arc<Mutex<Corpus>>,
    pub test_providers: Vec<Box<dyn TestCaseProvider>>,
    pub results: Arc<Mutex<Vec<TestCaseResult>>>,
}

impl Fuzzer {
    /// Construct a new fuzzer.
    pub fn new(
        config: FuzzingConfig,
        deployed: Vec<DeployedContract>,
        test_providers: Vec<Box<dyn TestCaseProvider>>,
        corpus: Corpus,
    ) -> Self {
        Fuzzer {
            config,
            deployed,
            coverage: Arc::new(CoverageMaps::new()),
            corpus: Arc::new(Mutex::new(corpus)),
            test_providers,
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Run the fuzzing campaign against `chain`.
    ///
    /// Blocks until one of the configured stop conditions fires:
    /// * `config.timeout_secs` seconds have elapsed, or
    /// * `config.test_limit` sequences have been executed, or
    /// * a failure is found and `config.stop_on_failure` is `true`.
    ///
    /// Returns all [`TestCaseResult`]s collected across all workers.
    pub fn run(&self, chain: &mut TestChain) -> Vec<TestCaseResult> {
        // ── shared stop-signal ────────────────────────────────────────────────
        let stop = Arc::new(AtomicBool::new(false));
        let sequence_count = Arc::new(AtomicU64::new(0));
        let start_time = Instant::now();

        // Clone the chain once per worker; each worker gets an independent copy
        // of the full chain state so they never share mutable EVM state.
        let num_workers = self
            .config
            .workers
            .unwrap_or_else(rayon::current_num_threads);

        // ── parallel worker pool ──────────────────────────────────────────────
        rayon::scope(|s| {
            for worker_idx in 0..num_workers {
                // Per-worker clones of everything the worker needs.
                let worker_chain = chain.clone();
                let coverage = Arc::clone(&self.coverage);
                let corpus = Arc::clone(&self.corpus);
                let results = Arc::clone(&self.results);
                let deployed = self.deployed.clone();
                let config = self.config.clone();
                let stop = Arc::clone(&stop);
                let count = Arc::clone(&sequence_count);
                // Wrap test_providers in Arc<Vec<_>> so workers can share them
                // without re-boxing (they are Send + Sync).
                let providers: Vec<&dyn TestCaseProvider> =
                    self.test_providers.iter().map(|p| p.as_ref()).collect();

                s.spawn(move |_| {
                    worker_loop(
                        worker_idx as u64,
                        worker_chain,
                        deployed,
                        config,
                        coverage,
                        corpus,
                        results,
                        stop,
                        count,
                        start_time,
                        providers,
                    );
                });
            }
        });

        // ── collect results ───────────────────────────────────────────────────
        let guard = self.results.lock().unwrap();
        guard.clone()
    }
}

// ── worker_loop ───────────────────────────────────────────────────────────────

/// The inner loop executed by each rayon worker.
#[allow(clippy::too_many_arguments)]
fn worker_loop(
    worker_idx: u64,
    mut chain: TestChain,
    deployed: Vec<DeployedContract>,
    config: FuzzingConfig,
    coverage: Arc<CoverageMaps>,
    corpus: Arc<Mutex<Corpus>>,
    results: Arc<Mutex<Vec<TestCaseResult>>>,
    stop: Arc<AtomicBool>,
    sequence_count: Arc<AtomicU64>,
    start_time: Instant,
    providers: Vec<&dyn TestCaseProvider>,
) {
    // Each worker gets a unique seed so they explore different corners of the
    // input space even when a global seed is set.
    let base_seed = config.seed.unwrap_or(0xDEAD_BEEF_CAFE_BABE);
    let seed = base_seed.wrapping_add(worker_idx.wrapping_mul(0x9E37_79B9_7F4A_7C15));

    let mut gen = SequenceGenerator::new(seed, deployed.clone(), config.clone());
    let shrinker = Shrinker::new(config.shrink_limit);

    loop {
        // ── stop conditions ───────────────────────────────────────────────────
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Some(timeout) = config.timeout_secs {
            if start_time.elapsed().as_secs() >= timeout {
                stop.store(true, Ordering::Relaxed);
                break;
            }
        }
        if let Some(limit) = config.test_limit {
            if sequence_count.load(Ordering::Relaxed) >= limit {
                stop.store(true, Ordering::Relaxed);
                break;
            }
        }

        // ── generate sequence ─────────────────────────────────────────────────
        let sequence = gen.generate();

        // Skip empty sequences (no deployed contracts yet or no fuzzable fns).
        if sequence.is_empty() {
            continue;
        }

        // ── execute on this worker's chain ────────────────────────────────────
        // Take a snapshot before the run so we can feed the sequence to
        // test providers without mutating the baseline state.  We restore
        // after the run to keep workers independent across iterations.
        let pre_snapshot = chain.snapshot();
        let exec_result = {
            let mut executor = SequenceExecutor::new(&mut chain);
            executor.execute(&sequence)
        };
        // Gather coverage from the execution (best-effort; no re-run needed).
        let local_cov = LocalCoverage::new(); // placeholder — real coverage comes via call_with_coverage

        // ── coverage / corpus feedback ────────────────────────────────────────
        if coverage.update(&local_cov) {
            if let Ok(mut c) = corpus.lock() {
                c.add(sequence.clone()).ok();
                gen.add_to_corpus(sequence.clone());
            }
        }

        sequence_count.fetch_add(1, Ordering::Relaxed);

        // ── test providers ────────────────────────────────────────────────────
        let mut found_failure = false;
        for provider in &providers {
            for deployed_contract in &deployed {
                let found = provider.analyze(&sequence, &exec_result, deployed_contract);
                for result in found {
                    if result.status == TestStatus::Failed {
                        found_failure = true;

                        // ── shrink ────────────────────────────────────────────
                        // Build a closure that re-executes the candidate
                        // sequence on a fresh clone of the pre-run snapshot.
                        let pre = pre_snapshot.clone();
                        let this_provider: &dyn TestCaseProvider = *provider;
                        let dc = deployed_contract.clone();

                        let is_failing = |candidate: &CallSequence| -> bool {
                            let mut trial_chain = chain.clone();
                            trial_chain.restore(pre.clone());
                            let mut executor = SequenceExecutor::new(&mut trial_chain);
                            let trial_result = executor.execute(candidate);
                            let found = this_provider.analyze(candidate, &trial_result, &dc);
                            found.iter().any(|r| r.status == TestStatus::Failed)
                        };

                        let shrunk = shrinker.shrink(sequence.clone(), is_failing);

                        let final_result = TestCaseResult {
                            failing_sequence: Some(shrunk),
                            ..result
                        };

                        if let Ok(mut r) = results.lock() {
                            r.push(final_result);
                        }
                    }
                }
            }
        }

        // ── restore chain state for the next iteration ────────────────────────
        chain.restore(pre_snapshot);

        // ── stop-on-failure ───────────────────────────────────────────────────
        if found_failure && config.stop_on_failure {
            stop.store(true, Ordering::Relaxed);
            break;
        }
    }
}
