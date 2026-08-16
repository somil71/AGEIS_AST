# Handoff Report: Sovereign Build Mode (R1) & Local-Only LLM Routing (R2) Specification

## 1. Observation

Direct observations from inspecting codebase files, dependencies, and build behavior:

### A. Root `Cargo.toml` and Dependency Analysis
- **File**: `d:\AEGIS_AST\Cargo.toml` (lines 12–101)
- Currently, `Cargo.toml` has no `[features]` table. All dependencies are compiled unconditionally by default.
- Active network/cloud dependencies identified:
  1. `sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "postgres", "macros"] }` (line 68) — connects to Postgres (Neon/Supabase) over TLS.
  2. `tower-cookies = "0.10"` (line 72) & `time = "0.3"` (line 73) — session cookie management for cloud OAuth.
  3. `reqwest = { version = "0.12", features = ["blocking", "json"] }` (line 79) — HTTP client used for cloud LLMs (Anthropic, OpenAI, Groq), GitHub OAuth, MCP cloud proxy, and Ollama.
  4. `axum = "0.7"` (line 61) & `tower-http = { version = "0.5", features = ["cors"] }` (line 62) — Web UI HTTP server and CORS layer.
  5. `open = "5"` (line 65) — launches system browser and shell handlers.
  6. `urlencoding = "2"` (line 76) — encodes URLs for GitHub OAuth redirects and cloud MCP queries.

### B. Server & Cloud Routes Codebase Analysis
- **`src/server/mod.rs`** (lines 1–5): Exposes `oauth`, `users`, `indexer`, `index_pipeline`.
- **`src/server/oauth.rs`** (lines 67–218): GitHub OAuth redirect (`/auth/github`), code exchange with `https://github.com/login/oauth/access_token`, user info fetch from `https://api.github.com/user`, repo listing from `https://api.github.com/user/repos`.
- **`src/server/users.rs`** (lines 23–48): Lazily initializes `PgPool` via `DATABASE_URL` / `DATABASE_URL_FALLBACK`, creates SQL tables (`users`, `sessions`, `user_repos`), manages API keys (`ndk_*`).
- **`src/server/indexer.rs`** (lines 13–126): Spawns a background worker polling the Postgres DB every 6 minutes, executes `git clone https://x-access-token:{token}@github.com/{repo}.git` to clone remote repos.
- **`src/cli/serve/mod.rs`** (lines 234–269): Registers 25 routes on Axum HTTP server including cloud routes (`/auth/github`, `/auth/callback`, `/auth/logout`, `/api/me`, `/api/repos`, `/api/import/github`).
- **`src/cli/mcp/mod.rs`** (lines 287–358): Functions `cloud_search` and `cloud_similar` make remote HTTP GET/POST calls to `NEEDLE_CLOUD_URL` with `NEEDLE_API_KEY`.

### C. LLM Routing Codebase Analysis
- **`src/llm.rs`** (lines 16–58):
  - `Provider` enum: `Anthropic`, `OpenAI`, `Groq`, `Ollama`.
  - `LlmClient::from_env()` checks `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GROQ_API_KEY` before falling back to `Ollama`.
  - Lines 99–119: Anthropic API call to `https://api.anthropic.com/v1/messages`.
  - Lines 139–158: OpenAI/Groq API call to `https://api.openai.com/v1/chat/completions` or `https://api.groq.com/openai/v1/chat/completions`.
  - Lines 172–188: Ollama call to `http://127.0.0.1:11434/api/chat`.

### D. Baseline Verification
- Executed `cargo test`: 0 unit tests passed; 0 failed; 0 ignored (baseline recorded).
- Executed `cargo tree`: current dependency tree pulls in `reqwest`, `hyper`, `tokio-rustls`, `sqlx-postgres`, `axum`, etc.

---

## 2. Logic Chain

1. **Isolation Requirement (R1)**:
   - For an air-gapped / sovereign defense application, compliance verification requires that `cargo tree --no-default-features --features sovereign` must prove the absolute absence of networking and cloud client crates.
   - Therefore, `axum`, `tower-http`, `tower-cookies`, `sqlx`, `reqwest`, `urlencoding`, and `open` must be declared as `optional = true` in `Cargo.toml`.
   - The default feature set must be `default = ["cloud"]`, enabling all optional crates to ensure zero behavioral regression for the default build.
   - The `sovereign` feature flag must compile the binary with zero default features (`--no-default-features --features sovereign`), ensuring none of the networking crates are linked.

2. **Local-Only LLM Loopback Architecture (R2)**:
   - When compiled with `--features sovereign`, external cloud providers (`Anthropic`, `OpenAI`, `Groq`) must be conditionally compiled out (`#[cfg(not(feature = "sovereign"))]`).
   - Communication with local Ollama at `http://127.0.0.1:11434` can be executed either via a lightweight localhost HTTP request over `tokio::net::TcpStream` / `std::net::TcpStream` (requiring zero external networking crates in `Cargo.toml`) or local CLI execution.
   - When the runtime `--offline-strict` flag is supplied:
     - The client enforces that target host IP resolves strictly to `127.0.0.1` or `::1`.
     - Any non-loopback URL or remote API call is immediately intercepted and rejected with `Error::OfflineStrictViolation`.
     - If Ollama is offline or unreachable on port 11434, the engine fails immediately with an explicit error, prohibiting any silent fallbacks.

3. **`needle doctor --sovereign` Verification Mechanism**:
   - The `needle doctor` command validates compile-time feature flags (`cfg!(feature = "sovereign")`), verifies that cloud endpoints and OAuth routes are disabled, audits environment variables for leaked cloud keys, probes `127.0.0.1:11434` for Ollama model availability, and verifies local cryptographic ledger integrity.

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Sovereign Build | `[features]` Table Configuration | Gating `axum`, `sqlx`, `reqwest`, `tower-cookies`, `open`, `urlencoding` under `cloud` feature; `sovereign` feature activates zero-network mode | Cargo build flags (`--no-default-features --features sovereign`) | Clean binary with no networking crates in `cargo tree` | Fails compilation if code accesses ungated network symbols | `Cargo.toml` analysis |
| 2 | Sovereign Build | `needle doctor --sovereign` | Diagnostic CLI command to verify zero-network compliance and system readiness | CLI flag `--sovereign`, optional `--offline-strict` | Formatted audit report table with `[PASS]`/`[FAIL]` status | Exits with code 1 if non-sovereign binary detected | `ORIGINAL_REQUEST.md` R1 |
| 3 | Sovereign Build | Cloud Route & Server Gating | Conditional compilation (`#[cfg(feature = "cloud")]`) disabling `/auth/github`, `/api/repos`, `/api/import/github`, `users.rs`, `oauth.rs` | `needle serve` invocation in sovereign mode | Informational message that `serve` is disabled in sovereign mode | Returns structured error on invalid command | `src/server/mod.rs`, `src/cli/serve/` |
| 4 | Sovereign Build | MCP Cloud Isolation | Disabling remote proxy calls (`cloud_search`, `cloud_similar`) in `needle mcp` when in sovereign mode | JSON-RPC requests via stdio | Local index results only | Fails gracefully if local index not initialized | `src/cli/mcp/mod.rs` |
| 5 | Local LLM | Sovereign Ollama Routing | `src/llm.rs` compiles exclusively with `Provider::Ollama` in sovereign build | `LlmClient::from_env()` or `LlmClient::new()` | Local completion from `127.0.0.1:11434/api/chat` | Returns explicit error if Ollama unreachable | `src/llm.rs` |
| 6 | Local LLM | Runtime `--offline-strict` Flag | Runtime flag rejecting all external/remote network calls and enforcing loopback-only communication | CLI flag `--offline-strict` / `LlmConfig.offline_strict` | Enforces loopback validation (`127.0.0.1`, `localhost`) | Throws `OfflineStrictViolation` on non-loopback calls | `ORIGINAL_REQUEST.md` R2 |
| 7 | Local LLM | Ollama Health Probe | Diagnostic probe checking `/api/tags` and model availability (e.g. `llama3.2`) | Local HTTP probe to `127.0.0.1:11434` | List of loaded models & readiness status | Returns descriptive model-missing prompt if model not pulled | `src/llm.rs` |
| 8 | Local Embedding | Hash-Projection & Local Embedding | Fallback to 384-dim pure CPU hash-projection or local Ollama embeddings | Text chunks | Unit-normalized `f32` vectors | Zero-dependency fallback guaranteed | `src/embedding/mod.rs` |

---

## 4. Edge Cases

| # | Feature | Input | Observed Behavior / Expected Specification |
|---|---------|-------|--------------------------------------------|
| 1 | `needle doctor --sovereign` | Invoked on binary compiled with default (cloud) features | Reports `[FAIL] Sovereign Mode: INACTIVE`, lists detected cloud dependencies (`sqlx`, `reqwest`, etc.), and exits with exit code `1`. |
| 2 | `needle serve` | Invoked on sovereign binary | Returns user-friendly message: `"needle serve is disabled in sovereign build mode. Use CLI commands (needle search, needle audit, needle doctor) or stdio MCP."` No panic. |
| 3 | Environment Hygiene | Cloud environment variables (`ANTHROPIC_API_KEY`, `DATABASE_URL`) set during sovereign execution | Sovereign binary ignores cloud variables. `needle doctor --sovereign` outputs informational warning reminding operator of unneeded credentials. |
| 4 | LLM `--offline-strict` | Ollama service is stopped/down when prompt is dispatched | Fails fast (within 2s) with explicit error: `"Error: Ollama is unreachable at 127.0.0.1:11434. In --offline-strict mode, external fallback is forbidden."` |
| 5 | LLM `--offline-strict` | Ollama model (e.g., `llama3.2`) is not pulled in Ollama | Explicit error: `"Model 'llama3.2' not found in local Ollama instance. Run: 'ollama pull llama3.2'"`. |
| 6 | LLM `--offline-strict` | User configures remote `OLLAMA_HOST=http://remote-server.com:11434` | Loopback validator rejects `remote-server.com` with `OfflineStrictViolation: Non-loopback host is forbidden under --offline-strict`. |
| 7 | Dependency Tree Verification | Running `cargo tree --no-default-features --features sovereign` | Output dependency graph contains 0 instances of `reqwest`, `hyper`, `sqlx`, `axum`, `tower-cookies`, `tower-http`, `urlencoding`, `open`. |

---

## 5. Caveats

- **Ollama Loopback Communication**: In sovereign mode with `--no-default-features --features sovereign`, `reqwest` is excluded from the build. Local Ollama HTTP requests must be handled via standard library `std::net::TcpStream` / `tokio::net::TcpStream` (raw minimal HTTP/1.1 client) to guarantee zero networking crates in `cargo tree`.
- **Constraint Compliance**: Files `src/embedding/mod.rs`, `src/indexing/bm25.rs`, and `src/indexing/hnsw.rs` must not be modified except for minimum feature-gating needed.

---

## 6. Conclusion

1. **R1 (Sovereign Build Mode)** is fully specified:
   - `Cargo.toml` features: `default = ["cloud"]`, `cloud = ["dep:axum", "dep:tower-http", "dep:tower-cookies", "dep:sqlx", "dep:reqwest", "dep:urlencoding", "dep:open"]`, `sovereign = []`.
   - `needle doctor --sovereign` verifies binary feature flags, cloud route absence, environment hygiene, Ollama loopback connectivity, and ledger state.
   - `cargo tree --no-default-features --features sovereign` guarantees zero networking crates.
2. **R2 (Local-Only LLM Routing)** is fully specified:
   - In sovereign build, `llm.rs` compiles out Anthropic, OpenAI, and Groq cloud providers.
   - Runtime `--offline-strict` strictly enforces loopback validation (`127.0.0.1`/`localhost`), rejects any external network calls with loud errors, and prohibits silent fallbacks.

---

## 7. Verification Method

To independently verify the implementation once coded:
1. **Verify Default Build Preserved**:
   ```bash
   cargo build --release
   cargo test
   ```
2. **Verify Sovereign Build & Dependency Tree**:
   ```bash
   cargo build --release --no-default-features --features sovereign
   cargo tree --no-default-features --features sovereign | Select-String "reqwest|hyper|sqlx|axum|tower"
   # Output must be empty
   ```
3. **Verify Sovereign Doctor Command**:
   ```bash
   cargo run --no-default-features --features sovereign -- doctor --sovereign
   # Must output [PASS] for all sovereign checks
   ```
4. **Verify Offline-Strict Rejection**:
   ```bash
   cargo run --no-default-features --features sovereign -- doctor --sovereign --offline-strict
   ```
