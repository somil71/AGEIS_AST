## 2026-08-14T18:31:00Z
You are the Sovereign & LLM Spec Miner for NEEDLE-SENTINEL.
Working directory: `d:\AEGIS_AST\.agents\spec_miner_sovereign_llm`
Project root: `d:\AEGIS_AST`
Original request path: `d:\AEGIS_AST\.agents\ORIGINAL_REQUEST.md`

First, read `d:\AEGIS_AST\.agents\ORIGINAL_REQUEST.md` completely.
Focus on Requirements R1 (Sovereign Build Mode) & R2 (Local-Only LLM Routing).
Your tasks:
1. Identify all network/cloud dependencies and code paths across the codebase:
   - Check `Cargo.toml` dependencies (reqwest, hyper, tokio-tungstenite, oauth, etc.).
   - Check `server/`, `server/users.rs`, auth routes, telemetry, remote endpoints.
   - Check `embedding/` and `indexing/` (remember constraint: do not modify `embedding/`, `indexing/bm25.rs`, `indexing/hnsw.rs` except minimum feature-gating needed).
2. Analyze R1 Sovereign Build Mode:
   - How `--features sovereign` should be structured in `Cargo.toml` (default vs `--no-default-features --features sovereign`).
   - What `needle doctor --sovereign` should check and output (verifying absence of network paths / dependencies).
   - How `cargo tree --no-default-features --features sovereign` can verify zero networking crates.
3. Analyze R2 Local-Only LLM Routing:
   - Inspect `src/llm.rs` (or equivalent LLM module in the codebase).
   - How routing to local Ollama works, and how the runtime `--offline-strict` flag should be implemented to reject network calls.
4. Enumerate all features, constraints, error cases, edge cases, and CLI/API signatures needed for R1 & R2.
5. Write your detailed specification report to `d:\AEGIS_AST\.agents\spec_miner_sovereign_llm\handoff.md` and send a completion message.

Constraints:
- Read-only spec mining: do NOT modify source code files.
