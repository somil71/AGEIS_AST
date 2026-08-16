# Scope: Milestone M1 (Sovereign Build Mode & Local-Only LLM Routing)

## Architecture
- Module/package boundaries:
  - `Cargo.toml`: Feature flags (`default = ["cloud"]`, `cloud = [...]`, `sovereign = []`). In sovereign mode, remote networking/cloud API dependencies are stripped out or optional.
  - `src/cli/doctor.rs`: Implement `needle doctor --sovereign` diagnostic command verifying feature flags, zero cloud routes, local Ollama connectivity (`/api/tags`), and ledger verification state.
  - `src/llm.rs`: Local-only Ollama LLM provider integration with strict loopback validation (`127.0.0.1`/`localhost`), rejecting remote endpoints when `--offline-strict` is active.
- Error handling: No `unwrap()`, `expect()`, or `panic!()` on user-facing or network paths; return structured `Result` / `DoctorError` / `LlmError`.

## Feature Inventory
| # | Feature | Description | Milestone | Status |
|---|---------|-------------|-----------|--------|
| F1 | Sovereign Cargo Features | `default = ["cloud"]`, `cloud = [...]`, `sovereign = []` in `Cargo.toml`. | M1 | Planned |
| F2 | Zero-Network Dependency Verification | `cargo tree --no-default-features --features sovereign` has 0 networking crates. | M1 | Planned |
| F3 | `needle doctor --sovereign` CLI | Diagnostic command checking feature flags, zero cloud routes, local Ollama connectivity, and ledger state. | M1 | Planned |
| F4 | Local-Only Ollama LLM Routing | Native Ollama client in `src/llm.rs` calling local `/api/generate` or `/api/chat`. | M1 | Planned |
| F5 | Runtime Loopback Validation | Strict `--offline-strict` enforcement ensuring LLM target IP is loopback (`127.0.0.1`/`localhost`) and rejecting all remote IP/domains. | M1 | Planned |
| F6 | Non-Panicking Error Propagation | Standardized error enums and clean propagation without unwrap/expect/panic on runtime paths. | M1 | Planned |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Sovereign Build Mode & Local-Only LLM Routing | Features F1-F6: Cargo.toml, src/cli/doctor.rs, src/llm.rs, tests | None | IN_PROGRESS |

## Interface Contracts
### `src/cli/doctor.rs`
- CLI flag `--sovereign` on `needle doctor`
- Output: structured checklist (Feature Flags, Cloud Routes, Ollama Local Connectivity, Ledger State)
- Exit code: 0 on success / all checks pass; non-zero if critical check fails.

### `src/llm.rs`
- Function signatures for Ollama local completions / embeddings
- Strict loopback URL validator: `validate_loopback_url(url: &str) -> Result<(), LlmError>`
- Conditional compilation `#[cfg(feature = "cloud")]` vs `#[cfg(feature = "sovereign")]`
