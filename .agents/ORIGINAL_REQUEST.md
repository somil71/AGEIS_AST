# Original User Request

## Initial Request — 2026-08-14T18:29:27Z

# Teamwork Project Prompt — Draft

> Status: Launched
> Goal: Craft prompt → get user approval → delegate to teamwork_preview

Build NEEDLE-SENTINEL: an extension of the existing Needle codebase that enables air-gapped AST-level code auditing and policy compliance checking, complete with a cryptographic audit ledger and local-only LLM routing.

Working directory: `d:\AEGIS_AST`
Integrity mode: demo

## Constraints
- **Branch Discipline**: Work exclusively on branch `feature/sentinel`. Never commit to `main`/`master`.
- **Pre-flight Check**: Confirm the NEEDLE repo is actually present in `d:\AEGIS_AST` before launching.
- **Baseline-First**: Run `cargo test` and record the pass/fail count before touching anything to establish ground truth.
- **File-touch Boundaries**: Do not modify `embedding/`, `indexing/bm25.rs`, `indexing/hnsw.rs` except for the minimum feature-gating needed.
- **Error Handling**: No `unwrap()`/`expect()`/`panic!()` on user-input paths (policy PDFs, source files).
- **Security**: The Ledger private key must never be logged, even at debug level.
- Pre-built libraries are OK for infra/crypto primitives per project brief §6.
- No copying external code for the compliance-graph or ledger logic specifically — those must be hand-rolled.
- No delegating execution to tools that could touch the network.

## Requirements

### R1. Sovereign Build Mode
Implement a compile-time feature flag (`--features sovereign`) that guarantees zero network capability by disabling all cloud/network routes (`server/users.rs`, OAuth) and dependencies. The default build must remain unchanged. Provide a `needle doctor --sovereign` command to verify the absence of network paths.

### R2. Local-Only LLM Routing
Ensure the `llm.rs` module routes exclusively to a local Ollama instance when compiled in sovereign mode. Include a runtime `--offline-strict` flag that explicitly rejects any network calls rather than silently failing.

### R3. Policy-Code Compliance Graph
Build a new `src/policy/` subsystem to parse policy PDFs/text (reusing `pdf-extract`), extract clauses, and use the local LLM to structure obligations. Link these policy obligations to code nodes via the existing `QueryEngine`. Expose this via a `needle audit` CLI command and new MCP tools (`get_obligations`, `check_compliance`, `get_compliance_report`).

### R4. Cryptographic Audit Ledger
Build an append-only JSONL ledger (`src/ledger/`) that uses `sha2` and `ed25519-dalek` to hash and sign compliance and security reports. It must provide CLI commands to `append` and `verify` the chain, catching and reporting any offline tampering.

## Acceptance Criteria

### Verification & Testing
- [ ] **Baseline**: Before modifying code, `cargo test` is run and its pass/fail count is recorded.
- [ ] `cargo build --release --no-default-features --features sovereign` compiles successfully. Verified via `cargo tree --no-default-features --features sovereign` showing no networking crate in the dependency tree, and a runtime smoke test showing zero connections.
- [ ] `cargo build --release` (default) produces a binary with identical behavior to the current master branch.
- [ ] `cargo test` and `cargo clippy --all-features -- -D warnings` pass without errors.
- [ ] The `needle doctor --sovereign` command successfully reports the absence of network features.
- [ ] A sample policy document can be ingested (`needle policy ingest`), and an audit (`needle audit`) produces a report linking policy clauses to code graph nodes.
- [ ] **Edge Case (R3)**: A scanned-image PDF with no extractable text fails loudly with a clear error, not silently indexing an empty document.
- [ ] **Edge Case (R4)**: A fresh/empty chain verifies cleanly (not erroring).
- [ ] Tampering with a signed ledger report causes `needle ledger verify` to fail and output the exact broken sequence number.
