# Progress: M1 Worker 1

**Last visited**: 2026-08-14T18:39:15Z

## Current Status: In Progress

### Tasks:
- [ ] 0. Baseline check: Run `cargo test` and record ground truth baseline.
- [ ] 1. Update `Cargo.toml`: Add `[features]`, configure `default = ["cloud"]`, `cloud = [...]`, `sovereign = []`, mark 8 cloud crates optional, add `sha2` and `ed25519-dalek`.
- [ ] 2. Update `src/error.rs`: Add `DoctorError`, `OfflineStrictViolation`, `LedgerError`, `PolicyError`.
- [ ] 3. Update `src/llm.rs`: Local-only Ollama client with raw async TCP HTTP client, `LoopbackValidator`, `LlmConfig`, gating cloud providers.
- [ ] 4. Module conditional gating:
  - `src/lib.rs`: gate `pub mod server;` under `#[cfg(feature = "cloud")]`
  - `src/cli/serve/`: gate web server under `#[cfg(feature = "cloud")]` and provide non-panicking informative message if invoked in sovereign mode
  - `src/embedding/mod.rs`: gate `Strategy::Ollama` `reqwest` usage under `#[cfg(feature = "cloud")]`
  - `src/cli/mcp/mod.rs`: handle cloud search gating cleanly
- [ ] 5. Implement `src/cli/doctor.rs`, register in `src/cli/mod.rs`, wire into `src/main.rs`.
- [ ] 6. Create tests in `tests/` verifying loopback validation, offline-strict enforcement, doctor command, and feature isolation.
- [ ] 7. Run verification builds and tests:
  - `cargo build` & `cargo test` (cloud default)
  - `cargo build --no-default-features --features sovereign` & `cargo test --no-default-features --features sovereign`
  - `cargo tree -p needle --no-default-features --features sovereign`
  - `cargo clippy --all-features -- -D warnings`
- [ ] 8. Write `handoff.md` and notify parent.
