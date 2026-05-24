# RESEARCH DOSSIER: Medusa

> Part of the Lancer ecosystem. A Rust clone of Trail of Bits' Medusa — parallelized coverage-guided smart contract fuzzer (Echidna successor). Build with Claude Code.
> Upstream: github.com/crytic/medusa (Go, AGPL-3.0, v1.0 Feb 2025). Built on go-ethereum's EVM.

## Feature Parity Mandate

**`lancer-medusa` MUST ship with 100% feature parity to the upstream `medusa` tool — every flag, every subcommand, every output format, every probe / detector / module / payload / mode — plus a native Rust GUI on top.** This is not a "subset clone" — this is a full reimplementation in Rust with a GUI wrapper.

### Parity contract

- **Every CLI flag** documented in upstream `medusa --help` (and the long-form man page / docs) must work in `lancer-medusa --cli`.
- **Every output format** the upstream tool emits (text, JSON, XML, CSV, SARIF, custom formats) must round-trip through `lancer-medusa` byte-equivalent where the format allows.
- **Every probe / detector / module / category / payload class / built-in script** must be ported. None silently dropped.
- **Every supported target type / input format / protocol / file format / language** must work.
- **Every configuration option** (config files, env vars, registry / settings entries) must be honoured.
- **Companion binaries / sub-tools** in the upstream suite (if any) must each get their own binary in this workspace.

### GUI on top

Every feature reachable via CLI is also reachable from the GUI (see `## GUI (Lancer-standard pattern)` below). The GUI is not a subset — it is an alternate frontend over the same `core` library that the CLI uses. The `core` crate is GUI-agnostic.

### Verification

- The `## Granular feature catalog` section below enumerates everything that must be supported.
- Reference: `tests/upstream_parity/` will pin upstream flag combinations + canned outputs as parity tests. CI fails if any flag is unsupported or output diverges.
- A `lancer-medusa --self-test` flag runs the full parity matrix against a bundled reference target and prints pass/fail per upstream feature group.

### What is allowed to diverge

- **Performance** — Rust-native, async-first; we are allowed to be faster.
- **Memory footprint** — we are allowed to be smaller.
- **GUI** — entirely new (no upstream GUI to copy except where the upstream ships its own GUI like Zenmap / Wireshark UI / Ghidra GUI / jadx-gui — in which case our GUI must cover every panel the upstream GUI has, plus the Lancer-standard layout from `## GUI` below).
- **AI augmentation** — net-new (see `## AI augmentation ideas`).

### What is NOT allowed to diverge

- Any flag silently dropped, renamed, or behavior-changed compared to upstream.
- Any output format / record schema diverging in field names, types, or semantics.
- Any built-in script / detector / module not ported (mark explicitly with `[N/A — see note]` if genuinely impossible in Rust, e.g. NSE Lua scripts requiring features not in `mlua`).

## What it actually does

Medusa (Trail of Bits, AGPL-3.0) is a parallelized, coverage-guided, mutational fuzzer for Solidity smart contracts. It's the Go-based successor to Echidna (Haskell). Built on `go-ethereum` for EVM execution, it supports three test modes — Property, Assertion, Optimization — across worker goroutines that share a coverage map and a corpus of interesting call sequences. v1.0 (Feb 2025) is the production release.

## Internal architecture

**Top-level layout:**
```
chain/            # Test chain wrapping go-ethereum's EVM
cmd/              # CLI (cobra)
  fuzz.go, fuzz_flags.go
  init.go, init_flags.go
  corpus.go, corpus_flags.go
  completion.go, root.go
  exitcodes/
compilation/      # solc/crytic-compile wrapper
events/           # Event/hook system
fuzzing/          # Core fuzzer (below)
logging/          # Structured logging
utils/            # Utilities
```

**`fuzzing/` package detail:**
- `fuzzer.go` — `Fuzzer` struct; orchestrator.
- `fuzzer_worker.go` — `FuzzerWorker` per OS thread/goroutine.
- `fuzzer_worker_sequence_generator.go` — generates call sequences (mixing corpus replay + mutation + fresh random).
- `fuzzer_worker_shrinking.go` — minimizes failing sequences.
- `fuzzer_events.go`, `fuzzer_hooks.go`, `fuzzer_metrics.go` — extensibility.
- `fuzzer_worker_events.go` — per-worker hooks.
- Subpackages:
  - `calls/` — `CallSequence`, `CallSequenceElement` (a `Tx` plus its decoded result).
  - `corpus/` — on-disk corpus (JSON files), loader, dedup, pruner.
  - `coverage/` — coverage maps (see below).
  - `config/` — `ProjectConfig`, `FuzzingConfig`, `TestingConfig`, etc.
  - `contracts/` — `Contract`, `ContractDefinition` from compilation artifacts.
  - `executiontracer/` — replay trace builder for failing sequences.
  - `reverts/` — per-function revert tally.
  - `valuegeneration/`:
    - `abi_values.go` — type-aware ABI value encoding.
    - `value_set.go` — pool of seed values (uint256, address, bytes).
    - `value_set_from_ast.go`, `value_set_from_slither.go` — initial seeding from contract literals and Slither constant analysis.
    - `generator_random.go`, `generator_mutational.go` — strategies.
    - `mutator.go`, `mutator_shrinking.go` — mutation operators & shrinker.
  - `testdata/contracts/` — known-good and known-bad fixtures.
- Per-mode files:
  - `test_case_property.go` / `_provider.go` — Echidna-style `echidna_*`/`property_*` boolean returners.
  - `test_case_assertion.go` / `_provider.go` — Solidity `assert(...)` failures and configured panic codes.
  - `test_case_optimization.go` / `_provider.go` — maximize an int256 returned from `optimize_*`.

**Fuzzer struct (key fields):**
```go
type Fuzzer struct {
    ctx, emergencyCtx           context.Context
    config                       config.ProjectConfig
    senders []common.Address
    deployer common.Address
    compilations  []types.Compilation
    contractDefinitions ContractDefinitions
    slitherResults *SlitherData

    baseValueSet *ValueSet            // seed values
    testCases    map[string]TestCase  // results
    workers      []*FuzzerWorker

    corpus        *corpus.Corpus       // on-disk + in-memory
    corpusPruner  *CorpusPruner
    revertReporter *reverts.RevertMetrics

    randomProvider *rand.Rand           // deterministic seed → forking
    testCasesLock  sync.Mutex
    Events, Hooks  ...
}
```

**Worker coordination.** `spawnWorkersLoop()`:
1. Buffered channel `threadReserveChannel` enforces `config.Workers` concurrency.
2. `availableWorkerSlotQueue` pre-forks RNG providers for determinism.
3. A worker drains a slot, runs a sequence (corpus item OR newly generated), updates global coverage map, returns slot.
4. After `WorkerResetLimit` sequences a worker tears down and a fresh one spawns to avoid memory bloat (Geth state objects accumulate).

**Coverage maps** (`fuzzing/coverage/coverage_maps.go`):
- Top-level `CoverageMaps` → per-contract `ContractCoverageMap`.
- Each `ContractCoverageMap` holds `executedMarkers map[uint64]uint64` (marker → hit count).
- Marker encoding (clever bit packing):
  - Jump: `(srcPC << 32) | dstPC`
  - Revert: `srcPC ^ 0x40000000`
  - Return: `srcPC ^ 0x80000000`
  - Contract entry: `0xC0000000 ^ firstPC`
- New marker (0→non-zero) triggers a "coverage changed" flag and the sequence is added to the corpus.
- Cross-worker merge via `Update()` is additive: `cm.executedMarkers[m] += count`.
- Per-thread cache (`cachedCodeAddress`, `cachedCodeHash`, `cachedMap`) avoids map lookups in the inner EVM loop.

## Granular feature list

**CLI subcommands:**
- `medusa fuzz` — run a fuzz campaign.
- `medusa init [hardhat|foundry|truffle]` — scaffold `medusa.json`.
- `medusa corpus prune` — drop redundant sequences from corpus.
- `medusa corpus health` — diagnostic.
- `medusa corpus clean` — **(v1.5.0, Feb 2025)** remove invalid corpus elements (stale ABI, removed functions, contract hash mismatch).
- `medusa completion {bash,zsh,fish,powershell}`.

**`medusa fuzz` flags** (from `cmd/fuzz_flags.go`):
| Flag | Default | Description |
|---|---|---|
| `--config` | `""` | medusa.json path |
| `--compilation-target` | `""` | foundry / hardhat / crytic-compile target |
| `--workers` | 0 (= NumCPU) | parallel workers |
| `--timeout` | 0 | seconds (0 = unlimited) |
| `--test-limit` | 0 | tx count cap |
| `--seq-len` | 0 | max calls in a sequence |
| `--target-contracts` | `[]` | contracts to fuzz |
| `--corpus-dir` | `""` | corpus & coverage output |
| `--senders` | `[]` | addresses with state-changing perms |
| `--deployer` | `""` | deployment address |
| `--no-color` | false | |
| `--fail-fast` | false | stop on first failure |
| `--explore` | false | exploration mode (no test mode) |
| `--use-slither` | false | use cached Slither data |
| `--use-slither-force` | false | re-run Slither |
| `--rpc-url` | `""` | on-chain fuzzing source |
| `--rpc-block` | 0 | block to fork from |
| `-v, --verbosity` | 0 | trace verbosity |
| `--log-level` | `""` | trace/debug/info/warn/error |

**Configuration file (`medusa.json`)** — every field group:

*Fuzzing:* `Workers`, `WorkerResetLimit`, `Timeout`, `TestLimit`, `ShrinkLimit`, `CallSequenceLength`.

*Coverage:* `CorpusDirectory`, `CoverageEnabled`, `CoverageFormats` (`["lcov","html"]`), `CoverageExclusions`, `PruneFrequency`.

*Testing:* `StopOnFailedTest`, `StopOnFailedContractMatching`, `StopOnNoTests`, `TestAllContracts`, `TestViewMethods`, `Verbosity`, `TargetFunctionSignatures`, `ExcludeFunctionSignatures`.

*AssertionTesting:* `Enabled`, `PanicCodeConfig` (toggle per panic code: 0x00, 0x01 assert, 0x11 over/underflow, 0x12 div-by-zero, 0x21, 0x22, 0x31 array under/over, 0x32, 0x41 OOM, 0x51 zero-fn-call).

*PropertyTesting:* `Enabled`, `TestPrefixes` (default `["fuzz_","property_","echidna_"]`).

*OptimizationTesting:* `Enabled`, `TestPrefixes` (default `["optimize_"]`).

*Target/deployment:* `TargetContracts`, `PredeployedContracts` (address → bytecode), `TargetContractsBalances`, `ConstructorArgs`, `DeployerAddress`, `SenderAddresses`.

*Chain:* `MaxBlockNumberDelay`, `MaxBlockTimestampDelay`, `TransactionGasLimit`.

*Compilation:* delegates to crytic-compile invocation: target (auto-detected foundry/hardhat), build args, solc remappings.

*Slither:* `UseSlither`, `CachePath`.

*Logging:* `Level`, `LogDirectory`, `NoColor`.

**Mutation strategies** (`generator_mutational.go` + `mutator.go`):
- *Integer mutations:* add small delta (±1..±256), flip random bits, replace with boundary (0, 1, type max, type max-1, signed min/max), replace with corpus value, replace with constant from contract.
- *Address mutations:* swap among `senders`, choose deployed contract address, zero address, random.
- *Bytes/string:* arithmetic on bytes (single-byte add/sub), bit flip, length grow/shrink, splice from another corpus bytes.
- *Array:* permute elements, drop, duplicate; new element from typed generator.
- *Dynamic struct/tuple:* per-field recursive mutation.

**Shrinking** (`mutator_shrinking.go` + `fuzzer_worker_shrinking.go`):
- Greedy. Iteratively (up to `ShrinkLimit` steps):
  1. Drop a call from the sequence; re-run; keep if still failing.
  2. Minimize each value (bisect to smaller absolute value; reduce string/bytes length; zero-out unused fields).
- Result attached to test-case failure report; HTML coverage marks the surviving calls.

**Test prefixes recognised** (Property mode by default):
- `fuzz_` (Medusa native)
- `property_` (Echidna compat)
- `echidna_` (Echidna compat)
- `assertion_` / `assert*` only in Assertion mode (any `assert(false)` etc.).
- `optimize_` for Optimization (returns int256; fuzzer maximizes return).
- `test*` for **function-level invariants** — a "unit test on steroids" style where the function takes fuzzer-controlled args and asserts pre/post-conditions (`testDeposit(uint256 _amount)`). Bounded with helpers like `clampLte()`. Distinct from contract-level system invariants.

**Two invariant categories (per upstream docs):**
- **Function-level invariants** — focused, single-function pre/post-condition tests.
- **System-level invariants** — global properties that must hold across any state (e.g. ERC20 `totalSupply == Σ balances`).

**Cheatcodes (full v1.5+ catalog — `vm.*`)** — 25+ implementations to port verbatim:
*Block/time:* `warp(uint256)`, `roll(uint256)`, `fee(uint256)`, `difficulty(uint256)` (deprecated), `prevrandao(bytes32)`, `chainId(uint256)`, `coinbase(address)`.
*Storage/code:* `store(address,bytes32,bytes32)`, `load(address,bytes32)`, `etch(address,bytes)`, `getCode(string)`, `getNonce(address)`, `setNonce(address,uint64)`.
*Identity:* `label(address,string)`, `deal(address,uint256)`, `addr(uint256)` (privKey→addr), `sign(uint256,bytes32)`.
*Prank:* `prank(address)`, `startPrank(address)`, `stopPrank()`, `prankHere(address)`.
*Snapshot:* `snapshot()` → uint256, `revertTo(uint256)`.
*FFI / parsing:* `ffi(string[])`, `toString(*)`, `parseBytes`, `parseBytes32`, `parseInt`, `parseUint`, `parseBool`, `parseAddress`.
*Assertion (v1.5.1, Mar 2025):* `assertEq`, `assertNotEq`, `assertGt`, `assertGte`, `assertLt`, `assertLte`, `assertTrue`, `assertFalse` — Foundry-style assertion helpers callable from within property/assertion tests.

**Console logging** — `console.log` / `console.logUint` / `console.logAddress` / `console.logBytes` / `console.logString` etc. surfaced in execution traces (Foundry-compatible `0x000000000000000000636F6e736F6c652e6c6f67` HEVM console address).

**Coverage report formats:**
- **HTML** (default) — per-file source highlighting, branch markers, hit counts, file explorer + search (v1.2.0+), surviving-shrunk-calls overlay.
- **LCOV** — `lcov.info` for IDE/CI integration (gcovr-compatible).
- **JSON** — machine-readable coverage map for downstream tools.
- Toggle via `coverage.formats: ["html","lcov","json"]`.

**Branch-coverage-guided mode** (v1.2.0 → v1.3.0): migrated from PC-based to **branch-based** coverage markers. JUMPI true/false arms tracked independently so a JUMPI hit only on one side keeps generating sequences toward the unhit side.

**Compilation targets supported** (delegated to `crytic-compile`):
- Foundry (auto-detect `foundry.toml`)
- Hardhat (`hardhat.config.js/ts`)
- Truffle
- Brownie
- Raw `solc` standard-JSON input
- Pre-compiled artifacts (`out/`, `artifacts/`)
- Library linking (full external library support, v1.3.0+)
- Hex / base-10 / scientific notation in contract balance literals (v1.2.0+).

**Pre-deployed contracts.** `PredeployedContracts: {address: bytecode}` — bake arbitrary contracts at chosen addresses before `setUp()`. Useful for simulating mainnet system contracts, oracles, multicall, gas tokens.

**Reverts reporter** (`reverts/`): per-function-selector tally of revert reasons across the campaign, printed as a table at end-of-run. Detects "you're hitting `Auth: not owner` 90% of the time → fuzz with the right sender".

**Slither integration depth:**
- Cached results in `slither.json` (configurable `Slither.CachePath`).
- Constants harvested from AST → seed pool.
- Function-call graph used to bias sequence generation toward state-mutating entry points.
- `--use-slither-force` re-runs Slither even if cache exists.
- Custom slither args via config (v1.1.0+).

**`fallback`/`receive` calling** (v1.4.0+) — fuzzer may emit raw value-transfer calls hitting `fallback()` / `receive()` of the target contract, important for proxy/router/forwarder fuzzing.

**Parallel corpus replay** (v1.4.0+) — startup corpus replay across all workers concurrently with immediate test-failure short-circuit (was sequential pre-v1.4).

**RPC fork support** (v1.1.0+) — `--rpc-url` + `--rpc-block` fork from mainnet/L2 at a pinned block; read-through cache in `chain/`; supports Alchemy/Infura/Anvil/Reth endpoints.

**Verbosity tiers (execution traces, v1.2.0+):**
- `-v 0` — silent.
- `-v 1` — show failing-sequence trace.
- `-v 2` — show failing trace + decoded events.
- `-v 3` — full per-call CALL/STATICCALL/DELEGATECALL/CREATE trace with arg decoding.

**Memory leak / state-bloat mitigations:**
- `WorkerResetLimit` (Geth state objects accumulate; recycle worker after N seqs).
- Corpus pruner with read-only locks for event callbacks (v1.5.0+).
- TrieDB and RPC-pool leak fixes (v1.5.1+).

**External integrations:**
- **`crytic/fuzz-utils`** — auto-generates Foundry unit-test reproducers from failed Medusa properties. The Rust port should ship a `lancer-medusa export-foundry <failure-json>` subcommand.
- **`crytic/secureum-medusa`** — Secureum training repo of Medusa exercises; useful regression set.

## Engineering hard parts

**Shrinking algorithm.** Greedy reduction is O(N²) over sequence length × shrink-limit; the hard part is preserving the failure deterministically — Medusa replays from a snapshotted post-`setUp()` state with the same RNG seed for sub-call values. Recent feedback (Trail of Bits internal benchmark) showed shrinking is the major bottleneck vs Echidna; v1.0 made it incremental but still trails Echidna's better shrinker.

**Coverage instrumentation.** Done at the EVM interpreter level, not by source instrumentation. Medusa patches `go-ethereum`'s `EVMInterpreter.Run()` with hooks at every JUMPI, RETURN, REVERT to update the marker map. The bit-packed encoding (high 2 bits identify event class, low 30 bits PC) avoids hash maps for hot paths.

**Fuzz→corpus feedback loop.**
1. Worker mutates a call sequence (either picks one from corpus and mutates, or generates fresh).
2. Replays under EVM with coverage inspector.
3. After execution, compares delta of coverage map against global.
4. If new marker found → sequence added to corpus, saved as JSON, broadcast via `Events`.
5. Other workers' next sequence-generation step is biased toward recent corpus additions.

**Parallelism gotchas.**
- Global coverage map updates need `sync.Mutex`, which becomes contended at high worker counts.
- Per-worker local map + periodic merge is the cure (Medusa uses both — workers maintain local then merge into global on coverage-change event).
- Memory leaks from accumulated Geth state are managed by `WorkerResetLimit` — a worker is recreated after N sequences.

**On-chain fuzzing.** When `--rpc-url` is set, deployment of test contract happens on a forked state (similar to Foundry). Cheatcodes are limited (no full Hardhat cheatcode parity), but `vm.warp`/`vm.roll`/`vm.deal`/`vm.prank` are supported. Forking state is read-through-cached by `chain/` package.

## Library mapping (Go)

| Need | Library |
|---|---|
| EVM | `github.com/ethereum/go-ethereum` |
| CLI | `github.com/spf13/cobra` |
| Config | `github.com/spf13/viper` (JSON) |
| Logging | custom + `github.com/rs/zerolog` |
| Compilation | crytic-compile (Python, shelled out) |
| Slither | shelled out (Python) |
| ABI | `go-ethereum/accounts/abi` |
| Randomness | `crypto/rand` for seeds, `math/rand` for mutation |
| Concurrency | stdlib `sync`, `context`, channels |
| Coverage HTML | embedded Go templates |

**For a Rust clone:** swap go-ethereum → `revm`; use `rayon` for workers (data parallelism) + `dashmap` for shared coverage; replace `cobra` with `clap`. The bit-packed coverage encoding ports directly. Solc invocation via `foundry-compilers`. Forking via revm's `Database` trait (Foundry's `Backend` reusable). Big win in Rust: zero-allocation EVM steps via revm, no per-sequence GC pressure.

## Validation test corpus

- **Medusa's `fuzzing/testdata/contracts/`** — every test mode has fixtures; assertion mode includes panic-code variants.
- **Echidna's example contracts** — Medusa supports the `echidna_*` prefix so all Echidna test fixtures work; great regression set.
- **Trail of Bits / consensys "not-so-smart-contracts"** repo — known-bad implementations (reentrancy, unprotected ownership, weak PRNG) that any fuzzer must trip on.
- **Badger DAO eBTC** — public 6-week fuzzing case study; Medusa caught actual production bugs.
- **devdacian's `solidity-fuzzing-comparison`** repo — head-to-head Foundry/Echidna/Medusa on the same property suites; useful clone benchmark.

## 2024–2026 state

- **2024:** alpha/beta; coverage HTML report added; on-chain fuzzing added; Slither integration for value seeding.
- **2025-02:** v1.0 announcement (Trail of Bits blog "Unleashing Medusa"). Production-grade. Direct HTML coverage feedback, parallel by default, Slither-augmented mutation. Benchmark vs Echidna: similar coverage on most tests; Medusa faster on parallel hardware; Echidna's shrinker still better on some.
- **2025–2026:** incremental shrinker improvements, optimization mode adoption, dashboards. Multi-block/time-aware mutators for DeFi-style sequencing tests.
- **Ecosystem alignment:** Foundry's invariant runner (v1.3 coverage-guided) borrows ideas from Medusa's coverage map; one direction of travel is convergence — Trident (Rust fuzzer) emerging.

> **Research note — 2026-05-30** (deep-dive vs upstream `crytic/medusa`):
> Verified current upstream is **v1.5.1 (Mar 11, 2025)** — production-stable, no v2 yet. Tag timeline confirmed: v1.1.0 (Feb 2024, RPC fork), v1.2.0 (Mar 2024, `startPrank`/`stopPrank`/`getCode` + branch coverage migration + HTML file-explorer/search), v1.3.0 (Jul 2024, branch-coverage finalization + `coverageExclusions` + `PruneFrequency` + external libraries), v1.4.0 (Dec 2024, parallel corpus replay + `fallback`/`receive` calling + HTML report fix), v1.5.0 (Feb 2025, `medusa corpus clean` + shrinker drops reverting txs + read-only event-callback locks), v1.5.1 (Mar 2025, `assert*` Foundry-style cheatcodes + integer-overflow fix + corpus-pruner/trieDB/RPC-pool memory leak fixes).
> Plan gaps closed in this pass: `corpus clean` subcommand; function-level vs system-level invariant distinction (`test*` prefix); full 25+ vm.* cheatcode catalog incl. v1.5.1 `assert*`; `console.log` HEVM-compatible logging; LCOV + JSON coverage output formats (HTML was already covered); **branch-coverage-guided** mode (vs the older PC-based — original plan only described PC-marker encoding); compilation targets list (Foundry/Hardhat/Truffle/Brownie/solc-standard-JSON); `PredeployedContracts` bake; reverts reporter; deeper Slither integration (AST constants, function-call-graph biasing, custom args); `fallback`/`receive` calling; parallel corpus replay; verbosity tiers 0-3 with event decoding; memory-leak/state-bloat mitigations; `fuzz-utils` Foundry-export companion; `secureum-medusa` regression corpus.
> The 2026 Venus-protocol DeFi-lending case study (`dev.to/ohmygod`) and the "Fuzzing DeFi Lending Invariants with Medusa" write-up confirm system-level invariant fuzzing as the dominant 2026 use-case — adding the function-level / system-level split to the plan is now strictly mandatory for parity with upstream `docs/src/testing/`.

## Real-world audit workflows

1. Auditor writes Echidna/Medusa-style `property_*` returners in a `tests/` contract.
2. Local quick run: `medusa fuzz --workers 4 --timeout 600` to validate setup.
3. Overnight: `medusa fuzz --workers 16 --timeout 28800 --corpus-dir corpus/`. Corpus persists between runs.
4. Failures inspected via HTML coverage + execution trace; shrinker produces a minimal repro.
5. Repro is hand-ported into a forge test for the finding writeup.
6. Cantina/Sherlock/Code4rena practice: a shared corpus directory across team members is committed to the contest scoping repo so everyone gets warm-started fuzzing.

## AI augmentation ideas

- **Invariant generation from ABI.** LLM reads contract ABI + NatSpec and proposes plausible `property_*` invariants (e.g., for ERC20: `totalSupply == sum(balances)`).
- **Mutation strategy LLM-bias.** When stuck on a coverage plateau, ask LLM what corner of the state space hasn't been explored ("did you try when `totalShares == 0`?"), translate to value-set additions.
- **Failure root-cause.** Failed sequence + trace → LLM produces human-readable summary.
- **Property scoring.** LLM ranks user-supplied invariants by "interesting-ness" to focus fuzz budget on weak ones.
- **Foundry export.** Auto-port a failing `medusa` sequence into a forge unit test scaffold.

## Implementation phases (10-week clone — Rust, leveraging revm)

**Week 1 — Skeleton.** Rust workspace; depend on revm (v27), alloy, clap, rayon, tokio. Module split mirroring Medusa (`chain/`, `cmd/`, `fuzzing/`).

**Week 2 — EVM chain wrapper.** Build `TestChain` around revm: deploy, call, snapshot/restore, block number/timestamp manipulation.

**Week 3 — Compilation & contract loader.** Wrap `foundry-compilers` to compile a Foundry project; load contract definitions with ABI + bytecode.

**Week 4 — Random + Mutational value generators.** ABI value generator: per-type random + mutate. Seed pool (`ValueSet`) from contract literals.

**Week 5 — Sequence generator + executor.** Build a `CallSequence`; execute each call; record per-call revert reason and return data.

**Week 6 — Coverage instrumentation.** Hook revm's `Inspector::step` to record JUMPI/RETURN/REVERT markers using Medusa's bit-packed encoding. DashMap-based shared coverage.

**Week 7 — Corpus + feedback.** On-disk JSON corpus. When new coverage marker → save sequence, broadcast on a tokio channel; sequence generator weights toward recent corpus.

**Week 8 — Test modes.** Property mode (`fuzz_`/`property_` prefix), Assertion mode (panic code config), Optimization mode (maximize int256).

**Week 9 — Shrinker + parallelism.** Greedy shrinker (drop calls, then minimize values). `rayon::scope` over workers; per-worker local coverage merged periodically.

**Week 10 — Reports.** HTML coverage report (template + per-contract line marking). Execution trace replay for failing sequences. CLI polish.

---

## GUI (Lancer-standard pattern)

`lancer-medusa` ships as a **native GUI application** by default. The CLI surface listed in `## Granular feature catalog` is still available behind a `--cli` flag (or via the `lancer-medusa-cli` companion binary), but the **plain `lancer-medusa` invocation opens a native desktop window**, matching every other tool in the Lancer ecosystem. No terminal-only mode.

### Framework

Pure-Rust GUI stack — no JavaScript, no Node, single static binary:

| Crate | Role |
|-------|------|
| `eframe` | Cross-platform window + event loop wrapper around `egui` |
| `egui` | Immediate-mode UI widgets (panels, tables, forms, menus, drag-and-drop) |
| `egui_extras` | Sortable tables, image loaders, syntax highlighting, date pickers |
| `egui_plot` | Charts, graphs, timelines (histograms, activity meters, progress) |
| `rfd` | Native file/folder pickers (open file, save dialog) |
| `directories` | Per-OS config dir (`~/.config/lancer-medusa/`, `%APPDATA%\lancer-medusa\`, etc.) |
| `image` | Screenshot / preview rendering inside egui |
| `arboard` | Clipboard read/write for "copy result" actions |
| `muda` | Native menu bar (Linux / macOS / Windows) |
| `egui-notify` | Toast notifications |

Optional dark/light theme persisted to `lancer-medusa-config.json` via `serde_json`.

### Window layout

Three-pane layout matching the Lancer mothership GUI:

```
┌─────────────────────────────────────────────────────────────────────┐
│  File   Tools   Settings   Help                              [≡]    │
├──────────┬────────────────────────────────────────┬─────────────────┤
│          │                                        │                 │
│  Inputs  │           Main view                    │  Detail pane    │
│  / jobs  │  (table of results / source tree /     │  (full record / │
│  / scope │   findings list / packet list / etc.)  │   bytes / src / │
│          │                                        │   remediation)  │
│          │                                        │                 │
├──────────┴────────────────────────────────────────┴─────────────────┤
│  Status: idle           Items: 0          [Open]  [Run]  [Export]   │
└─────────────────────────────────────────────────────────────────────┘
```

- **Left sidebar (200-300px)** — inputs / queued jobs / scope / target list. Drag-and-drop accepted.
- **Center pane (flexible)** — primary result view appropriate to the tool (table for findings, tree for hierarchical data, code editor for source).
- **Right pane (300-400px, collapsible)** — full detail of the currently-selected item.
- **Top menu bar** — File (Open / Save / Export / Recent / Quit), Tools (per-tool actions), Settings (theme, fonts, integrations), Help (About / Docs).
- **Bottom status bar** — current activity, item counts, primary action buttons.

### Cross-tool integration with Lancer + the other 29 clones

Every Lancer-clone GUI exposes a small local HTTP server on a port auto-allocated at launch (`127.0.0.1:<port>`, written to `%TEMP%/lancer-medusa.port`):

- The main Lancer GUI discovers running tools by scanning `%TEMP%/lancer-*.port` and lists them in its sidebar.
- Right-clicking any item in any tool can "Send to <other Lancer tool>" — the receiving tool fetches the payload via its own HTTP endpoint.
- A future "Lancer Hub" tray shell can launch / focus any of the 30 GUIs from one menu.
- HTTP endpoints exposed:
  - `GET  /v1/state` — current jobs / progress
  - `POST /v1/import` — accept a target / file / record from another Lancer tool
  - `GET  /v1/export?format=json|csv|sarif|markdown` — pull the current result set in a portable format

### Async work + responsiveness

- Long-running scans run on a Tokio task pool; the GUI thread only renders.
- Progress propagates via `crossbeam::channel` from worker → UI.
- A `Stop` button cancels in-flight work via a `CancellationToken`.
- All file I/O / network I/O / subprocess work goes through async channels — the egui thread never blocks.

### Build output

The existing `build.bat` / `build-windows.bat` / `build-linux.sh` already produce a single binary. With `eframe` linked, the same binary is the GUI app.

- Windows: `build\lancer-medusa.exe` (double-click opens the GUI window).
- Linux: `build/lancer-medusa` (also installable as a `.desktop` entry via `build-linux.sh --install`).
- macOS: `build/lancer-medusa` (bundled into `lancer-medusa.app` via `cargo-bundle` in the `build-mac.sh` companion).

For CI / headless / scripting, pass `--cli` to fall through to the CLI surface from `## Granular feature catalog`.

## Build Setup (Lancer-standard pattern)

This clone follows the same build pattern as the main Lancer project — single `.exe` output via `build.bat` on Windows, `.so/elf` via `build-linux.sh` on Linux.

### Project layout

```
lancer-medusa/
├── Cargo.toml              # single-crate Rust project
├── Cargo.lock              # committed (binary, not library)
├── src/
│   └── main.rs             # entry point (scaffold)
├── build.bat               # Windows build → build/lancer-medusa.exe
├── build-windows.bat       # Windows packaged dist build → dist/lancer-medusa-windows-x86_64/
├── build-linux.sh          # Linux build → build/lancer-medusa
├── .gitignore              # plan.md (local-only) + target/ + build/ + dist/
├── README.md               # public-facing (pushed to GitHub)
└── plan.md                 # this file (gitignored — local blueprint, never pushed)
```

### Build commands

**Windows (double-click `build.bat`):**
```
build.bat
```
Produces `build\lancer-medusa.exe`. Pauses at end so the window stays open.

**Windows (packaged dist):**
```
build-windows.bat
```
Produces `dist\lancer-medusa-windows-x86_64\bin\lancer-medusa.exe` plus LICENSE/README — ready to zip.

**Linux / macOS:**
```bash
./build-linux.sh
```
Produces `build/lancer-medusa`.

### Requirements
- Rust toolchain (https://rustup.rs)
- Windows: Visual C++ Build Tools 2019/2022 (needed by some crates)

### CI

The build pattern is intentionally identical across Lancer and all 19 clones, so a single Jenkins/GitHub Actions matrix can build everything by calling `build-windows.bat` or `build-linux.sh` per repo.
