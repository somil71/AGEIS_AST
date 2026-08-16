# NEEDLE-SENTINEL: Codebase & Baseline Survey Report

## 1. Observation

### 1.1 Repo Pre-flight & Layout Verification
- **Project Root**: `d:\AEGIS_AST`
- **Workspace Manifest**: `d:\AEGIS_AST\Cargo.toml` contains `[workspace] members = [".", "src-tauri"]`, package name `needle` (v0.1.0, edition 2021).
- **Core Dependencies**:
  - AST / Chunking: `tree-sitter` (0.20), `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-typescript`, `tree-sitter-go`, `tree-sitter-java`, `tree-sitter-cpp`, `tree-sitter-php`, `pdf-extract` (0.7), `xxhash-rust` (0.8).
  - Runtime / Async: `tokio` (1.35), `rayon` (1.7), `crossbeam` (0.8), `axum` (0.7), `tower-http` (0.5), `tower-cookies` (0.10).
  - Network / Cloud DB: `reqwest` (0.12), `sqlx` (0.8 with postgres/rustls), `uuid` (1), `urlencoding` (2).
  - Storage & Serialization: `serde` (1.0), `serde_json` (1.0), `toml` (0.8), `bincode` (1), `memmap2` (0.9).
  - CLI: `clap` (4.4 with derive), `colored` (2), `indicatif` (0.17), `tracing` (0.1).
- **Directory Layout**:
  - `src/` (45 Rust source files):
    - `src/main.rs`: CLI entry point parsing subcommands (`Init`, `Search`, `Status`, `Reindex`, `Config`, `Bench`, `Watch`, `Mcp`, `Serve`, `Report`, `Graph`).
    - `src/lib.rs`: Library root exposing modules `analysis`, `chunking`, `config`, `embedding`, `error`, `graph`, `indexing`, `llm`, `query`, `schema`, `storage`, `server`, `watcher`.
    - `src/cli/`: Subcommand handlers (`init.rs`, `search.rs`, `status.rs`, `reindex.rs`, `config.rs`, `bench.rs`, `watch.rs`, `mcp/`, `serve/`, `report.rs`, `graph.rs`).
    - `src/server/`: Server modules (`mod.rs`, `users.rs`, `oauth.rs`, `indexer.rs`, `index_pipeline.rs`).
    - `src/indexing/`: Search indexing engines (`bm25.rs`, `hnsw.rs`, `mod.rs`).
    - `src/query/`: Query engine & ranking fusion (`mod.rs`, `fusion.rs`).
    - `src/graph/`: Knowledge graph extraction & analytics (`mod.rs`, `extract_scripting.rs`, `extract_rs_go.rs`, `extract_java_cpp.rs`).
    - `src/chunking/`: Tree-sitter & prose chunkers (`mod.rs`, `code.rs`, `prose.rs`).
    - `src/embedding/`: Embedding generation (`mod.rs` supporting hash-projection & Ollama `nomic-embed-text`).
    - `src/llm.rs`: Multi-provider LLM client (`Anthropic`, `OpenAI`, `Groq`, `Ollama`).
    - `src/analysis/`: Static analysis on code graph (`mod.rs`, `churn.rs`, `security.rs`).
    - `src/storage/`: Index persistence & serialization (`mod.rs`).
    - `src/watcher/`: File change watcher (`mod.rs`).
    - `src/config.rs`, `src/error.rs`, `src/schema.rs`.
  - `src-tauri/`: Tauri v2 desktop shell wrapper.
  - `benches/`: Micro-benchmarks (`bm25_bench.rs`, `hnsw_bench.rs`, `embedding_bench.rs`).
  - `docs/` & `design/`: PRDs, architectural diagrams, screenshots, schema specifications.

### 1.2 Git Branch Status
- **Initial Status**: Checked out on `main` tracking `origin/main`.
- **Command Executed**: `git checkout -B feature/sentinel`
- **Active Branch**: `feature/sentinel`
- **Confirmation**: `git branch` returned `* feature/sentinel`.

### 1.3 Baseline Ground Truth (`cargo test`)
- **Command Executed**: `cargo test`
- **Toolchain**: `stable-x86_64-pc-windows-gnu` / rustc 1.96.0
- **Compile Time**: 37.65s (debug profile [unoptimized + debuginfo])
- **Execution Results**:
  - `Running unittests src\lib.rs`:
    ```
    running 0 tests
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    ```
  - `Running unittests src\main.rs`:
    ```
    running 0 tests
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    ```
  - `Doc-tests needle`:
    ```
    running 0 tests
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
    ```
- **Total Test Counts**:
  - Total tests executed: `0`
  - Passed: `0`
  - Failed: `0`
  - Ignored: `0`
  - Doctests: `0`
  - Unit tests: `0`
- **Exit Code**: `0`

### 1.4 Baseline Compilation & Lint Status (`cargo check` & `cargo clippy`)
- **Command Executed**: `cargo check`
  - Result: `Finished dev profile [unoptimized + debuginfo] target(s) in 1.11s`, Exit code `0`.
- **Command Executed**: `cargo clippy`
  - Result: `Finished dev profile [unoptimized + debuginfo] target(s) in 11.00s`, Exit code `0`.
  - Warnings breakdown:
    - `needle` (lib): 16 clippy warnings (mostly `unnecessary_sort_by`, `collapsible_if`, `manual_strip`, `io_other_error`, `ptr_arg`, `double_ended_iterator_last`, `while_let_loop`).
    - `needle` (bin "needle"): 20 clippy warnings (`unnecessary_sort_by`, `double_ended_iterator_last`, `too_many_arguments`, `io_other_error`).
    - No compilation errors in library or main binary.
- **Command Executed**: `cargo check --all-targets` (specifically testing `--benches`)
  - Finding: Benchmark files `benches/hnsw_bench.rs` and `benches/bm25_bench.rs` contain outdated API signatures (`search_knn` argument mismatch and missing `score` method on `BM25Index`), but the main workspace crate targets (`lib`, `bin`) build cleanly.

---

## 2. Logic Chain

1. **Pre-flight & Workspace Confirmation**:
   - We inspected `Cargo.toml` and verified all required modules and dependencies exist in `d:\AEGIS_AST`.
   - The directory tree conforms to the Needle architecture specification.
2. **Branch Isolation**:
   - Per requirement constraints, changes must never be committed to `main`.
   - Creating and verifying `feature/sentinel` ensures safe development on the designated feature branch.
3. **Baseline Ground Truth**:
   - Running `cargo test` on the unmodified codebase established the ground truth baseline: the existing codebase currently contains 0 unit/doc tests. All new tests written for NEEDLE-SENTINEL will establish positive regression coverage.
   - Running `cargo check` and `cargo clippy` confirmed that the library and main binary compile without errors and with 36 known informational lints.
4. **Architectural Mapping for NEEDLE-SENTINEL**:
   - **R1 (Sovereign Build Mode)**:
     - Cloud/network routes and dependencies are concentrated in `src/server/users.rs` (`sqlx`), `src/server/oauth.rs` (`reqwest`, `oauth`), `src/server/indexer.rs`, and cloud handlers in `src/cli/serve/handlers_import.rs`.
     - In `Cargo.toml`, networking/database dependencies (`sqlx`, `tower-cookies`, `time`, `open`) can be gated behind a default feature (`cloud` or `default`), while `--features sovereign` disables them.
     - A new `needle doctor --sovereign` subcommand can inspect active feature flags and report zero network dependencies.
   - **R2 (Local-Only LLM Routing & Offline Strictness)**:
     - `src/llm.rs` currently supports Anthropic, OpenAI, Groq, and Ollama.
     - In sovereign mode / `--offline-strict`, external API clients (Anthropic, OpenAI, Groq) must be compiled out or rejected at runtime, routing exclusively to local Ollama (`http://127.0.0.1:11434`), failing loudly if network egress is attempted.
   - **R3 (Policy-Code Compliance Graph Subsystem)**:
     - Pure Rust PDF text extraction is already available via `pdf-extract` (used in `src/cli/init.rs:84` and `src/server/index_pipeline.rs:39`).
     - A new module `src/policy/` will ingest policy PDFs/text, validate non-empty extractable text (erroring explicitly on empty scanned images), parse clauses, query the AST graph using `QueryEngine`, and emit compliance reports.
     - Expose via `needle audit` CLI subcommand and MCP tools (`get_obligations`, `check_compliance`, `get_compliance_report`) in `src/cli/mcp/`.
   - **R4 (Cryptographic Audit Ledger Subsystem)**:
     - A new `src/ledger/` module will manage an append-only JSONL ledger signed with `ed25519-dalek` and hashed with `sha2`.
     - Ledger commands `needle ledger append` and `needle ledger verify` will catch offline tampering, output broken sequence numbers on corrupted entries, and verify empty/fresh chains cleanly.

---

## 3. Caveats

- **Existing Tests**: Zero unit tests existed prior to this survey. All testing verification for SENTINEL must introduce comprehensive automated test suites (`tests/sentinel_tests.rs` or unit tests within new modules).
- **Benchmarks Target**: Benchmark files under `benches/` have outdated signatures against recent `BM25Index` and `HnswIndex` refactorings. When testing with `cargo test` or `cargo check`, do not run `--benches` unless updating benchmarks is in scope.
- **Tauri Crate**: `src-tauri` is a member of the workspace, but `needle` CLI/core library operates independently.

---

## 4. Conclusion

- **Pre-flight**: Fully verified and validated.
- **Git Branch**: Successfully created and active on `feature/sentinel`.
- **Baseline State**:
  - `cargo test`: 0 total tests, 0 failures, exit code 0.
  - `cargo check`: Clean compilation (1.11s).
  - `cargo clippy`: Clean with 0 errors and 36 known lints.
- **Readiness**: The repository is in a clean, consistent state and ready for Phase 1 architectural design and subsystem implementation of NEEDLE-SENTINEL.

---

## 5. Verification Method

To independently verify all findings in this survey report:

1. **Verify Git Branch**:
   ```powershell
   git status
   git branch --show-current
   # Expected: feature/sentinel
   ```
2. **Verify Baseline Test Execution**:
   ```powershell
   cargo test
   # Expected:
   # running 0 tests (lib.rs) -> ok. 0 passed; 0 failed
   # running 0 tests (main.rs) -> ok. 0 passed; 0 failed
   # running 0 tests (doctests) -> ok. 0 passed; 0 failed
   ```
3. **Verify Baseline Compilation**:
   ```powershell
   cargo check
   # Expected: Finished dev profile, exit code 0
   ```
4. **Verify Baseline Linting**:
   ```powershell
   cargo clippy
   # Expected: Finished dev profile with warnings, exit code 0
   ```
