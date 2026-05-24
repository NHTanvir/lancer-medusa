use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use alloy_primitives::Bytes;
use serde::{Deserialize, Serialize};

// ── Internal serde types for solc --combined-json output ──────────────────────

#[derive(Deserialize)]
struct SolcOutput {
    contracts: HashMap<String, SolcContract>,
}

#[derive(Deserialize)]
struct SolcContract {
    abi: serde_json::Value,
    bin: String,
    #[serde(rename = "bin-runtime")]
    bin_runtime: String,
}

// ── Public types ───────────────────────────────────────────────────────────────

/// A compiled Solidity contract with its ABI and bytecode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDefinition {
    pub name: String,
    pub abi: alloy_json_abi::JsonAbi,
    /// Deployment (creation) bytecode, hex-decoded.
    pub bytecode: Bytes,
    /// Runtime (deployed) bytecode, hex-decoded.
    pub deployed_bytecode: Bytes,
    /// Path of the source file this contract was compiled from, if known.
    pub source_file: Option<String>,
}

// ── Compiler ───────────────────────────────────────────────────────────────────

pub struct Compiler;

impl Compiler {
    /// Compile a single `.sol` file using the `solc` binary found on `PATH`.
    /// Returns one [`ContractDefinition`] per contract defined in the file.
    pub fn compile_file(sol_path: &Path) -> anyhow::Result<Vec<ContractDefinition>> {
        Self::compile_file_with("solc", sol_path)
    }

    /// Compile a single `.sol` file using a specific `solc` binary path.
    pub fn compile_file_with(
        solc: &str,
        sol_path: &Path,
    ) -> anyhow::Result<Vec<ContractDefinition>> {
        // ── 1. Run solc ────────────────────────────────────────────────────────
        let output = Command::new(solc)
            .args([
                "--combined-json",
                "abi,bin,bin-runtime",
                sol_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("sol_path contains invalid UTF-8"))?,
            ])
            .output()
            .map_err(|e| {
                anyhow::anyhow!(
                    "solc not found: failed to execute '{}': {}",
                    solc,
                    e
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("solc exited with non-zero status:\n{}", stderr);
        }

        // ── 2. Parse JSON ──────────────────────────────────────────────────────
        let solc_out: SolcOutput = serde_json::from_slice(&output.stdout)
            .map_err(|e| anyhow::anyhow!("failed to parse solc JSON output: {}", e))?;

        // ── 3. Convert each contract entry ─────────────────────────────────────
        let mut definitions = Vec::with_capacity(solc_out.contracts.len());

        for (key, contract) in solc_out.contracts {
            // key format:  "FileName.sol:ContractName"
            let name = key
                .rsplit(':')
                .next()
                .unwrap_or(&key)
                .to_string();

            // The source file is the part before the last ':'
            let source_file = key.rsplit_once(':').map(|(file, _)| file.to_string());

            // Hex-decode bytecode; empty bin == interface/abstract contract
            let bytecode = decode_hex_bytes(&contract.bin).map_err(|e| {
                anyhow::anyhow!("failed to decode bin for contract '{}': {}", name, e)
            })?;

            let deployed_bytecode = decode_hex_bytes(&contract.bin_runtime).map_err(|e| {
                anyhow::anyhow!(
                    "failed to decode bin-runtime for contract '{}': {}",
                    name,
                    e
                )
            })?;

            // Parse ABI
            let abi: alloy_json_abi::JsonAbi = serde_json::from_value(contract.abi)
                .map_err(|e| {
                    anyhow::anyhow!("failed to parse ABI for contract '{}': {}", name, e)
                })?;

            definitions.push(ContractDefinition {
                name,
                abi,
                bytecode,
                deployed_bytecode,
                source_file,
            });
        }

        Ok(definitions)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Decode a hex string (with or without `0x` prefix) to [`Bytes`].
/// Returns `Bytes::new()` for an empty string (interface contracts).
fn decode_hex_bytes(hex_str: &str) -> anyhow::Result<Bytes> {
    let s = hex_str.trim();
    if s.is_empty() {
        return Ok(Bytes::new());
    }
    let s = s.strip_prefix("0x").unwrap_or(s);
    let raw = hex::decode(s)?;
    Ok(Bytes::from(raw))
}
