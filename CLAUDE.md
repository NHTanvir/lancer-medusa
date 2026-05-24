# CLAUDE.md

Guidance for Claude Code (and humans) working in this repository.

## What this is

`lancer-medusa` — A Rust clone of the `medusa` smart-contract fuzzer, built as part of the Lancer ecosystem — a complete 2026 AI-era bug bounty toolkit. It performs coverage-guided fuzzing against EVM smart contracts using an embedded `revm` execution engine. The tool is currently in active development (scaffold phase), with implementation details tracked in the local `plan.md`.

## Architecture

- `src/main.rs` — Binary entry point; CLI argument parsing via `clap`
- `src/report.rs` — Fuzzing result reporting and output formatting
- `src/chain/mod.rs` — EVM chain state management (backed by `revm`)
- `src/compilation/mod.rs` — Smart contract compilation and ABI loading (`alloy-json-abi`, `alloy-sol-types`)
- `src/fuzzing/mod.rs` — Top-level fuzzer orchestration module
- `src/fuzzing/fuzzer.rs` — Core fuzzer loop and campaign management
- `src/fuzzing/sequence_executor.rs` — Executes transaction sequences against the chain
- `src/fuzzing/sequence_generator.rs` — Generates candidate transaction sequences
- `src/fuzzing/corpus/` — Corpus storage and management for interesting inputs
- `src/fuzzing/coverage/` — Coverage tracking and instrumentation
- `src/fuzzing/calls/` — Call generation and mutation helpers
- `src/fuzzing/contracts/` — Contract-under-test representation
- `src/fuzzing/config/` — Per-run fuzzer configuration
- `src/fuzzing/valuegeneration/` — Value generators for fuzzing inputs
- `src/fuzzing/test_case*.rs` — Test case types: basic, assertion, property, optimization, shrinking

## Conventions

- Error propagation uses `anyhow::Result`; no `unwrap`/`expect` on fallible I/O.
- GUI is gated behind the default `gui` feature; `--no-default-features` builds a headless CLI binary.
- Library code lives in `src/lib.rs` (when present); binary entry point in `src/main.rs`.
- Async runtime is `tokio` (full features); CPU-bound parallel work uses `rayon`.
- Shared concurrent state uses `dashmap`; serialization uses `serde` + `serde_json`.
- EVM primitives (`Address`, `U256`, etc.) come from `alloy-primitives`; do not introduce conflicting ethereum crates.
- Structured logging uses the `tracing` crate; configure log level via the `RUST_LOG` environment variable.

## Build / test / run

```
build.bat                   # Windows → build\lancer-medusa.exe
build-windows.bat           # Windows packaged dist → dist\lancer-medusa-windows-x86_64\bin\lancer-medusa.exe
./build-linux.sh            # Linux/macOS
cargo build --release       # standard release build
cargo test                  # unit + integration tests
```

## Repo notes

- `plan.md` is local-only and must NOT be committed. It is gitignored; if ever tracked, run `git rm --cached plan.md`.
- `.claude/` and `target/`/`build/`/`dist/` are local-only and gitignored.
