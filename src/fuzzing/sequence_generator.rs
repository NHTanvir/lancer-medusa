#![allow(dead_code)]

use alloy_primitives::{keccak256, U256};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use crate::fuzzing::calls::{CallSequence, FuzzCall};
use crate::fuzzing::config::FuzzingConfig;
use crate::fuzzing::contracts::DeployedContract;
use crate::fuzzing::valuegeneration::{AbiValue, MutationalGenerator, RandomGenerator, ValueSet};

// ---------------------------------------------------------------------------
// SequenceGenerator
// ---------------------------------------------------------------------------

/// Generates call sequences for fuzzing, using both random generation and
/// corpus-guided mutation.
pub struct SequenceGenerator {
    rng: StdRng,
    random_gen: RandomGenerator,
    mutational_gen: MutationalGenerator,
    deployed: Vec<DeployedContract>,
    /// Corpus of interesting sequences to replay and mutate.
    corpus: Vec<CallSequence>,
    config: FuzzingConfig,
}

impl SequenceGenerator {
    pub fn new(seed: u64, deployed: Vec<DeployedContract>, config: FuzzingConfig) -> Self {
        let value_set = ValueSet::with_defaults(&[]);
        SequenceGenerator {
            rng: StdRng::seed_from_u64(seed),
            random_gen: RandomGenerator::new(seed.wrapping_add(1), value_set.clone()),
            mutational_gen: MutationalGenerator::new(seed.wrapping_add(2), value_set),
            deployed,
            corpus: Vec::new(),
            config,
        }
    }

    /// Generate a new call sequence.
    ///
    /// When the corpus is non-empty there is a 50 % chance of producing a
    /// mutated replay of an existing corpus entry; otherwise a fresh random
    /// sequence is generated.
    pub fn generate(&mut self) -> CallSequence {
        if !self.corpus.is_empty() && self.rng.gen_bool(0.5) {
            self.generate_mutated()
        } else {
            self.generate_fresh()
        }
    }

    /// Add a sequence to the corpus (typically called when new coverage is found).
    pub fn add_to_corpus(&mut self, seq: CallSequence) {
        self.corpus.push(seq);
    }

    /// Generate a fully random sequence of `config.call_sequence_length` calls.
    fn generate_fresh(&mut self) -> CallSequence {
        let len = self.config.call_sequence_length;
        let mut seq = CallSequence::new();
        for _ in 0..len {
            if let Some(call) = self.random_call() {
                seq.push(call);
            }
        }
        seq
    }

    /// Pick a corpus sequence and return a mutated copy of it.
    fn generate_mutated(&mut self) -> CallSequence {
        // Safety: caller guarantees corpus is non-empty.
        let idx = self.rng.gen_range(0..self.corpus.len());
        let base = self.corpus[idx].clone();

        let mut seq = CallSequence::new();
        for call in base.calls {
            let mutated_args: Vec<AbiValue> = call
                .args
                .into_iter()
                .map(|v| self.mutational_gen.mutate(v))
                .collect();

            seq.push(FuzzCall {
                from: call.from,
                to: call.to,
                selector: call.selector,
                args: mutated_args,
                value: call.value,
                function_name: call.function_name,
            });
        }
        seq
    }

    /// Pick a random deployed contract and one of its fuzzable functions,
    /// then generate random arguments for that function.
    fn random_call(&mut self) -> Option<FuzzCall> {
        if self.deployed.is_empty() {
            return None;
        }

        // Pick a random contract.
        let contract_idx = self.rng.gen_range(0..self.deployed.len());
        let contract = &self.deployed[contract_idx];

        let fuzzable = contract.fuzzable_functions();
        if fuzzable.is_empty() {
            return None;
        }

        // Pick a random function.
        let func_idx = self.rng.gen_range(0..fuzzable.len());
        let func = &fuzzable[func_idx];

        // Compute the 4-byte selector from keccak256(signature).
        let sig = func.signature();
        let hash = keccak256(sig.as_bytes());
        let selector: [u8; 4] = hash[..4].try_into().unwrap();

        // Generate random arguments for each input parameter.
        let args: Vec<AbiValue> = func
            .inputs
            .iter()
            .map(|param| self.random_gen.generate_for_type(param))
            .collect();

        // Use a zero address as the default caller; real callers can be set by
        // higher-level orchestration once wallet management is in place.
        let from = alloy_primitives::Address::ZERO;
        let to = contract.address;

        Some(FuzzCall::new(from, to, selector, args, &func.name))
    }
}
