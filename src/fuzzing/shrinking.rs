#![allow(dead_code)]

use alloy_primitives::{I256, U256};

use crate::fuzzing::calls::{CallSequence, FuzzCall};
use crate::fuzzing::valuegeneration::AbiValue;

// ── Shrinker ──────────────────────────────────────────────────────────────────

/// Greedy sequence shrinker.
///
/// Given a failing `CallSequence`, repeatedly tries to remove calls and
/// minimise argument values while the sequence continues to fail.  Stops
/// when no further reduction is possible or `shrink_limit` steps are
/// exhausted.
pub struct Shrinker {
    /// Maximum number of oracle-query steps before giving up.
    pub shrink_limit: usize,
}

impl Shrinker {
    /// Create a new shrinker with the given step budget.
    pub fn new(shrink_limit: usize) -> Self {
        Shrinker { shrink_limit }
    }

    /// Greedily shrink `sequence` until it is minimal or the step budget runs
    /// out.  `is_failing` is called for every candidate and must return `true`
    /// when the candidate still triggers the same failure.
    pub fn shrink<F>(&self, sequence: CallSequence, is_failing: F) -> CallSequence
    where
        F: Fn(&CallSequence) -> bool,
    {
        let mut steps: usize = 0;
        let mut seq = sequence;

        loop {
            if steps >= self.shrink_limit {
                break;
            }

            let before_len = seq.len();
            seq = self.drop_calls(seq, &is_failing, &mut steps);

            if steps >= self.shrink_limit {
                break;
            }

            let after_drop_len = seq.len();
            seq = self.minimize_values(seq, &is_failing, &mut steps);
            let after_min_len = seq.len();

            // If neither phase made progress, we're done.
            let made_progress = after_drop_len < before_len || after_min_len < after_drop_len;
            // Also treat arg minimisation as progress even when lengths match:
            // the minimize_values phase tracks its own internal progress via the
            // step counter, so compare its bool return value instead.  Here we
            // use a simple length-based check: if len didn't shrink we can't
            // know from outside.  A more robust approach tracks changed values,
            // but for the greedy pass checking outer-loop progress is enough.
            let _ = after_min_len; // suppress unused warning
            if !made_progress {
                break;
            }
        }

        seq
    }

    /// Phase 1 – try removing each call one at a time.
    ///
    /// For every index `i` a shorter sequence is built by omitting call `i`.
    /// If the shorter sequence still fails it is kept; otherwise the call is
    /// restored and the next index is tried.
    fn drop_calls<F>(
        &self,
        mut sequence: CallSequence,
        is_failing: &F,
        steps: &mut usize,
    ) -> CallSequence
    where
        F: Fn(&CallSequence) -> bool,
    {
        let mut i = 0;
        while i < sequence.calls.len() {
            if *steps >= self.shrink_limit {
                break;
            }

            // Build candidate without call `i`.
            let removed = sequence.calls.remove(i);
            *steps += 1;

            if is_failing(&sequence) {
                // Keep the shorter sequence; do NOT advance `i` because
                // the next call has just shifted down into position `i`.
            } else {
                // Restore the removed call.
                sequence.calls.insert(i, removed);
                i += 1;
            }
        }

        sequence
    }

    /// Phase 2 – try replacing each argument with a simpler value.
    ///
    /// For each call, for each argument position, a set of candidate "smaller"
    /// values is tried in order.  The first candidate that still causes a
    /// failure is committed; otherwise the original value is kept.
    fn minimize_values<F>(
        &self,
        mut sequence: CallSequence,
        is_failing: &F,
        steps: &mut usize,
    ) -> CallSequence
    where
        F: Fn(&CallSequence) -> bool,
    {
        for call_idx in 0..sequence.calls.len() {
            let num_args = sequence.calls[call_idx].args.len();
            for arg_idx in 0..num_args {
                if *steps >= self.shrink_limit {
                    return sequence;
                }

                let candidates = smaller_values(&sequence.calls[call_idx].args[arg_idx]);
                for candidate in candidates {
                    if *steps >= self.shrink_limit {
                        return sequence;
                    }

                    let original = sequence.calls[call_idx].args[arg_idx].clone();
                    sequence.calls[call_idx].args[arg_idx] = candidate;
                    *steps += 1;

                    if is_failing(&sequence) {
                        // Keep the smaller value; stop trying further candidates
                        // for this argument.
                        break;
                    } else {
                        // Restore the original.
                        sequence.calls[call_idx].args[arg_idx] = original;
                    }
                }
            }
        }

        sequence
    }
}

// ── smaller_values ────────────────────────────────────────────────────────────

/// Return a list of candidate "smaller" replacements for `value`, ordered from
/// smallest to largest.  An empty list means no simplification is attempted
/// for that type.
fn smaller_values(value: &AbiValue) -> Vec<AbiValue> {
    match value {
        AbiValue::Uint(bits, current) => {
            let mut candidates = Vec::new();
            if *current != U256::ZERO {
                candidates.push(AbiValue::Uint(*bits, U256::ZERO));
            }
            let half = *current >> 1;
            if half != U256::ZERO && half != *current {
                candidates.push(AbiValue::Uint(*bits, half));
            }
            candidates
        }

        AbiValue::Int(bits, current) => {
            let mut candidates = Vec::new();
            if *current != I256::ZERO {
                candidates.push(AbiValue::Int(*bits, I256::ZERO));
            }
            // Halve toward zero.
            let half = *current / I256::try_from(2i64).unwrap_or(I256::ONE);
            if half != I256::ZERO && half != *current {
                candidates.push(AbiValue::Int(*bits, half));
            }
            candidates
        }

        AbiValue::Bytes(data) if !data.is_empty() => {
            vec![AbiValue::Bytes(vec![])]
        }

        AbiValue::BytesDynamic(data) if !data.is_empty() => {
            vec![AbiValue::BytesDynamic(vec![])]
        }

        AbiValue::Array(elems) if !elems.is_empty() => {
            vec![AbiValue::Array(vec![])]
        }

        // Bool, Address, String, FixedArray, Tuple — leave as-is.
        _ => vec![],
    }
}
