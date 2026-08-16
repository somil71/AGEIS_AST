# Handoff Report: Specification for `needle doctor --sovereign` & Diagnostic Subsystem (Milestone M1)

## 1. Observation

Direct observations from inspecting the codebase, configuration, and interface contracts:

### A. CLI Command Routing and Entry Points
- **`src/main.rs`** (lines 10–98): The `Cli` struct parses `Commands` enum containing `Init`, `Search`, `Status`, `Reindex`, `Config`, `Bench`, `Watch`, `Mcp`, `Serve`, `Report`, `Graph`.
  - Currently, `Doctor` subcommand is **missing** from `enum Commands`.
  - Line 111 dispatches subcommands; missing handler for `Commands::Doctor`.
- **`src/cli/mod.rs`** (lines 1–12): Exports CLI sub-modules: `bench`, `config`, `graph`, `init`, `mcp`, `reindex`, `report`, `search`, `serve`, `status`, `watch`.
  - `pub mod doctor;` is currently absent and must be added.
- **`src/cli/status.rs`** (lines 1–103): Demonstrates project CLI styling patterns using `colored::Colorize` (`.bold()`, `.green()`, `.yellow()`, `.cyan()`, `.dimmed()`).

### B. Dependency & Build Configuration
- **`Cargo.toml`** (lines 12–101):
  - Currently, all dependencies are unconditional.
  - Network crates to be gated under `cloud` feature: `axum`, `tower-http`, `tower-cookies`, `sqlx`, `reqwest`, `open`, `urlencoding`.
  - In sovereign build (`--no-default-features --features sovereign`), `reqwest` is stripped out. Therefore, any loopback network probe in `doctor` must either use raw `std::net::TcpStream` or tokio TCP sockets, guaranteeing 0 networking crates in `cargo tree`.

### C. Error Handling Foundation
- **`src/error.rs`** (lines 6–17): `Error` enum defines `Io`, `InvalidPath`, `IndexNotFound`, `ChunkingError`, `EmbeddingError`, `IndexError`, `QueryError`, `ConfigError`, `SerializationError`, `Other`.
  - Missing variants: `DoctorError(String)`, `OfflineStrictViolation(String)`, `LedgerError(String)`.

### D. LLM and Storage Layout
- **`src/llm.rs`** (lines 16–58, 161–188): Ollama default port is `127.0.0.1:11434`. Endpoint for models inspection is `/api/tags`.
- **`src/storage/mod.rs`** (lines 33–68): Project root `.needle/` directory convention; ledger audit path is `.needle/ledger/audit_chain.jsonl`.

---

## 2. Logic Chain

1. **Sovereign Isolation Mandate (R1)**:
   - For an air-gapped system, verification must prove both compile-time feature isolation (`cfg!(feature = "sovereign")` AND `cfg!(not(feature = "cloud"))`) and zero cloud routes.
   - `needle doctor --sovereign` is the authoritative diagnostic tool for this proof.
2. **Zero-Networking Dependency Constraint**:
   - In sovereign mode, `reqwest` is not compiled. The diagnostic probe to Ollama (`http://127.0.0.1:11434/api/tags`) must use standard library `std::net::TcpStream` (or `tokio::net::TcpStream`) with raw HTTP/1.1 wire protocol to avoid introducing networking dependencies into the sovereign tree.
3. **Loopback Enclosure Verification**:
   - Strict loopback validation ensures all LLM traffic is constrained to `127.0.0.1`, `localhost`, `[::1]`, or `127.0.0.0/8`. Any remote IP or hostname must be rejected.
4. **Cryptographic Ledger Clean Verification (R4 & AC)**:
   - Per Acceptance Criteria: a fresh or uninitialized ledger chain must verify cleanly (0 entries, `[✓] PASS`), while corrupted or tampered chains must pinpoint the exact sequence number that failed.
5. **Deterministic Exit Codes & Structured Diagnostics**:
   - Exit code `0` on all checks passing, `1` on sovereign compliance or readiness failure, `2` on CLI parameter errors.
   - Structured visual output with `[✓]` (green), `[✗]` (red), `[!]` (yellow), and `[i]` (cyan), plus summary table and machine-readable JSON support.

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | CLI Command | `needle doctor` Subcommand | CLI diagnostic entry point supporting `--sovereign`, `--offline-strict`, `--ollama-url`, `--ledger-path`, `--json` | CLI arguments | Formatted terminal diagnostic report or JSON | Returns Exit Code 1 on failure, 2 on arg error | `ORIGINAL_REQUEST.md` R1 |
| 2 | Diagnostic Check | Compile Feature Flag Verification | Audits `cfg!(feature = "sovereign")` and `cfg!(feature = "cloud")` | Compile flags | Status `PASS`/`FAIL` with linked dependency details | Reports non-sovereign build when `--sovereign` requested | `PROJECT.md` F2 |
| 3 | Diagnostic Check | Cloud Routes & Server Gating | Verifies remote Axum routes (`/auth/github`, `/api/repos`) and OAuth are compiled out | Compile state | Status `PASS`/`FAIL` | Fails if cloud endpoints are accessible | `src/server/mod.rs` |
| 4 | Diagnostic Check | Environment Credential Hygiene | Checks for active cloud API keys (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DATABASE_URL`) | Environment variables | Status `PASS`/`WARN` | Flags unneeded credentials in air-gapped run | `src/llm.rs` |
| 5 | Diagnostic Check | Local Ollama Connectivity Probe | Raw TCP HTTP/1.1 probe to `/api/tags` with 2000ms timeout (zero third-party networking crates) | Target loopback URL (`127.0.0.1:11434`) | Status `PASS`/`FAIL`/`WARN` + discovered models | Explicit failure if connection refused/timeout | `ORIGINAL_REQUEST.md` R1/R2 |
| 6 | Diagnostic Check | Model Readiness Audit | Parses `/api/tags` JSON payload to verify required models (e.g. `llama3.2`) are present | Ollama response JSON | Status `PASS`/`WARN` with remediation command | Warns if 0 models or required model missing | `src/llm.rs` |
| 7 | Diagnostic Check | Loopback URL Strict Validator | Verifies URL resolves strictly to `127.0.0.1`, `localhost`, `[::1]`, `127.0.0.0/8` | URL string | `Ok(())` or `Err(OfflineStrictViolation)` | Fails with `OfflineStrictViolation` on remote IP | `SCOPE.md` F5 |
| 8 | Diagnostic Check | Cryptographic Ledger Integrity Check | Verifies `.needle/ledger/audit_chain.jsonl` integrity, sequence continuity, and signature chain | Path to ledger file | Status `PASS`/`FAIL` + block count | Fails with exact broken sequence # on tamper | `PROJECT.md` F19/F20 |
| 9 | Diagnostic Check | Clean/Empty Chain Verification | Verifies fresh or non-existent ledger without error (0 blocks verified) | Non-existent or empty ledger path | Status `PASS` (0 blocks, clean state) | Clean pass, no error thrown | `ORIGINAL_REQUEST.md` AC |
| 10 | Diagnostic Check | Local Storage & Index Health | Verifies local `.needle/` index structure and chunk storage accessibility | Path to `.needle/` | Status `PASS`/`INFO` + chunk count & disk size | Informative status if uninitialized | `src/storage/mod.rs` |
| 11 | Output Formatting | Diagnostics Summary Table & Exit Codes | ASCII summary table with Check, Status, Details; Checkmarks `[✓]`, `[✗]`, `[!]`; Exit code 0, 1, 2 | Verification results | Rendered ANSI terminal output / exit code | Sets process exit code via `std::process::exit` | `PROJECT.md` F2 |

---

## 4. Edge Cases

| # | Feature | Input | Observed Behavior / Expected Specification |
|---|---------|-------|--------------------------------------------|
| 1 | Feature Flag Check | `needle doctor --sovereign` on binary compiled with default (`cloud`) features | Fails with `[✗] Compile-Time Mode: NON-SOVEREIGN`. Details list active cloud crates (`axum`, `sqlx`, `reqwest`). Summary shows `FAILED`. Exit code `1`. |
| 2 | Feature Flag Check | `needle doctor --sovereign` on binary compiled with `--no-default-features --features sovereign` | Passes with `[✓] Compile-Time Mode: SOVEREIGN`. Details confirm zero cloud crates linked. |
| 3 | Ollama Offline | Ollama daemon not running (port 11434 closed) | Fails fast (< 2000ms) with `[✗] Local LLM Routing: Ollama unreachable at 127.0.0.1:11434 (Connection refused)`. Remediation: `"Run 'ollama serve'"`. Exit code `1` under `--sovereign`. |
| 4 | Ollama Model Missing | Ollama running, but no models installed (`/api/tags` returns `{"models":[]}`) | Outputs `[!] Local LLM Routing: Ollama online at 127.0.0.1:11434, but 0 models found`. Remediation: `"Run 'ollama pull llama3.2'"`. |
| 5 | Remote Host Configured | User sets `OLLAMA_HOST=http://192.168.1.100:11434` or `--ollama-url https://api.openai.com` | Loopback validator rejects input with `[✗] Non-loopback host rejected under sovereign mode`. Exit code `1`. |
| 6 | Fresh/Empty Ledger | `.needle/ledger/audit_chain.jsonl` does not exist or is 0 bytes | Passes cleanly with `[✓] Cryptographic Ledger: Chain intact (0 blocks, clean state)`. Zero error, exit code `0`. |
| 7 | Tampered Ledger | Byte modified at line 3 of `audit_chain.jsonl` | Fails with `[✗] Cryptographic Ledger: Tamper detected at sequence #2 (Payload hash mismatch)`. Exit code `1`. |
| 8 | Cloud Environment Variables | `ANTHROPIC_API_KEY` set during sovereign execution | Outputs `[!] Environment Hygiene: ANTHROPIC_API_KEY detected in env (ignored in sovereign mode)`. Remediation: `"Unset environment variable for clean air-gap"`. |
| 9 | Invalid Ledger Path | User passes `--ledger-path /invalid/nonexistent/dir/file.jsonl` | Fails with `[✗] Ledger Error: Cannot read parent directory`. Exit code `1` or `2`. No panic. |
| 10 | Malformed JSON on Probe | Ollama port occupied by non-HTTP or non-Ollama service returning garbage | Probe returns error: `"Invalid response from port 11434: Failed to parse JSON"`. Handled cleanly via `Result::Err`, zero panic. |

---

## 5. Implementation Specification Guide

### Step 1: Update CLI Definitions (`src/main.rs` and `src/cli/mod.rs`)

1. In `src/cli/mod.rs`, add:
   ```rust
   pub mod doctor;
   ```

2. In `src/main.rs`, update `enum Commands`:
   ```rust
   /// Run diagnostic checks for system readiness and sovereign mode compliance
   Doctor {
       /// Verify sovereign mode (zero cloud routes, loopback LLM, ledger integrity)
       #[arg(long)]
       sovereign: bool,

       /// Strict offline validation (reject any non-loopback endpoint)
       #[arg(long)]
       offline_strict: bool,

       /// Ollama endpoint to probe (default: http://127.0.0.1:11434)
       #[arg(long, default_value = "http://127.0.0.1:11434")]
       ollama_url: String,

       /// Custom path to ledger audit chain file
       #[arg(long)]
       ledger_path: Option<String>,

       /// Output diagnostics as JSON
       #[arg(long)]
       json: bool,
   },
   ```

3. In `src/main.rs` dispatch:
   ```rust
   Commands::Doctor { sovereign, offline_strict, ollama_url, ledger_path, json } => {
       cli::doctor::run(sovereign, offline_strict, &ollama_url, ledger_path.as_deref(), json).await?;
   }
   ```

---

### Step 2: Error Variant Additions (`src/error.rs`)

Add variants to `Error`:
```rust
#[derive(Debug)]
pub enum Error {
    // Existing variants...
    Io(std::io::Error),
    InvalidPath(String),
    IndexNotFound(String),
    ChunkingError(String),
    EmbeddingError(String),
    IndexError(String),
    QueryError(String),
    ConfigError(String),
    SerializationError(String),
    Other(Box<dyn std::error::Error>),
    // New Sentinel / Sovereign variants:
    DoctorError(String),
    OfflineStrictViolation(String),
    LedgerError(String),
    PolicyError(String),
}
```

---

### Step 3: Implement `src/cli/doctor.rs` Architecture

```rust
//! `needle doctor` — Diagnostic and sovereign readiness verification subsystem.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Fail,
    Warn,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: CheckStatus,
    pub details: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub sovereign_mode: bool,
    pub offline_strict: bool,
    pub checks: Vec<DiagnosticCheck>,
}

impl DoctorReport {
    pub fn is_success(&self) -> bool {
        !self.checks.iter().any(|c| c.status == CheckStatus::Fail)
    }

    pub fn print_human(&self) {
        println!("{}", "Needle v0.1.0 — Sovereign Readiness Diagnostic\n".bold());

        for check in &self.checks {
            let symbol = match check.status {
                CheckStatus::Pass => "[✓]".green().bold(),
                CheckStatus::Fail => "[✗]".red().bold(),
                CheckStatus::Warn => "[!]".yellow().bold(),
                CheckStatus::Info => "[i]".cyan().bold(),
            };
            println!("{} {:<26} {}", symbol, check.name.bold(), check.details);
            if let Some(ref rem) = check.remediation {
                if check.status == CheckStatus::Fail || check.status == CheckStatus::Warn {
                    println!("    {} {}", "↳ Fix:".dimmed(), rem.yellow());
                }
            }
        }

        println!("\n{}", "=".repeat(80).dimmed());
        println!("{:<32} {:<10} {}", "Check".bold(), "Status".bold(), "Details".bold());
        println!("{}", "-".repeat(80).dimmed());

        for check in &self.checks {
            let status_str = match check.status {
                CheckStatus::Pass => "PASS".green(),
                CheckStatus::Fail => "FAIL".red().bold(),
                CheckStatus::Warn => "WARN".yellow(),
                CheckStatus::Info => "INFO".cyan(),
            };
            println!("{:<32} {:<10} {}", check.name, status_str, check.details);
        }
        println!("{}", "-".repeat(80).dimmed());

        let total = self.checks.len();
        let passed = self.checks.iter().filter(|c| c.status == CheckStatus::Pass).count();
        let failed = self.checks.iter().filter(|c| c.status == CheckStatus::Fail).count();
        let warnings = self.checks.iter().filter(|c| c.status == CheckStatus::Warn).count();

        if self.is_success() {
            println!(
                "Overall Result: {} ({}/{} checks passed{})",
                "PASSED".green().bold(),
                passed,
                total,
                if warnings > 0 { format!(", {} warnings", warnings) } else { "".into() }
            );
            println!("{}", "Status: System is fully ready for sovereign air-gapped operation.\n".green());
        } else {
            println!(
                "Overall Result: {} ({} failed, {} warnings, {} passed)",
                "FAILED".red().bold(),
                failed,
                warnings,
                passed
            );
            println!("{}", "Status: System is NOT ready for sovereign air-gapped operation.\n".red().bold());
        }
    }
}
```

#### Core Diagnostic Check Functions in `src/cli/doctor.rs`:

1. **Loopback URL Validation**:
   ```rust
   pub fn validate_loopback_url(url_str: &str) -> crate::Result<(String, u16)> {
       let trimmed = url_str
           .trim_start_matches("http://")
           .trim_start_matches("https://");
       let mut parts = trimmed.splitn(2, '/');
       let host_port = parts.next().unwrap_or(trimmed);
       let mut hp = host_port.splitn(2, ':');
       let host = hp.next().unwrap_or("127.0.0.1");
       let port: u16 = match hp.next() {
           Some(p) => p.parse().unwrap_or(11434),
           None => 11434,
       };

       let is_loopback = host == "localhost"
           || host == "127.0.0.1"
           || host == "::1"
           || host == "[::1]"
           || host.starts_with("127.");

       if !is_loopback {
           return Err(crate::Error::OfflineStrictViolation(format!(
               "Non-loopback host '{}' rejected under sovereign / --offline-strict mode",
               host
           )));
       }
       Ok((host.to_string(), port))
   }
   ```

2. **Raw TCP Loopback Probe (Zero networking crate dependency)**:
   ```rust
   pub fn probe_raw_http_get(host: &str, port: u16, path: &str, timeout_ms: u64) -> Result<(u16, String), String> {
       let host_resolved = if host == "localhost" { "127.0.0.1" } else { host };
       let addr_str = format!("{}:{}", host_resolved, port);
       let addrs: Vec<SocketAddr> = addr_str
           .to_socket_addrs()
           .map_err(|e| format!("Cannot resolve {}: {}", addr_str, e))?
           .collect();

       if addrs.is_empty() {
           return Err(format!("No socket addresses for {}", addr_str));
       }

       let stream = TcpStream::connect_timeout(&addrs[0], Duration::from_millis(timeout_ms))
           .map_err(|e| format!("Connection to {} failed: {}", addr_str, e))?;

       stream.set_read_timeout(Some(Duration::from_millis(timeout_ms))).ok();
       stream.set_write_timeout(Some(Duration::from_millis(timeout_ms))).ok();

       let mut stream = stream;
       let request = format!(
           "GET {} HTTP/1.1\r\nHost: {}:{}\r\nUser-Agent: needle-doctor/0.1.0\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
           path, host, port
       );

       stream.write_all(request.as_bytes()).map_err(|e| format!("Write failed: {}", e))?;
       stream.flush().map_err(|e| format!("Flush failed: {}", e))?;

       let mut response = Vec::new();
       stream.read_to_end(&mut response).map_err(|e| format!("Read failed: {}", e))?;

       let response_str = String::from_utf8_lossy(&response);
       let mut parts = response_str.splitn(2, "\r\n\r\n");
       let headers = parts.next().unwrap_or("");
       let body = parts.next().unwrap_or("");

       let status_code = headers
           .lines()
           .next()
           .and_then(|line| {
               let mut parts = line.split_whitespace();
               parts.next()?; // HTTP/1.1
               parts.next()?.parse::<u16>().ok()
           })
           .unwrap_or(0);

       Ok((status_code, body.to_string()))
   }
   ```

3. **Check Implementations**:
   - `check_compile_features(sovereign_flag: bool) -> DiagnosticCheck`:
     - Evaluates `cfg!(feature = "sovereign")` vs `cfg!(feature = "cloud")`.
     - In sovereign mode: PASS if `sovereign` is enabled and `cloud` is disabled.
     - If non-sovereign and `--sovereign` flag is passed: FAIL with remediation to recompile.
   - `check_cloud_routes() -> DiagnosticCheck`:
     - Evaluates whether remote web routes and OAuth endpoints are active.
   - `check_environment_hygiene() -> DiagnosticCheck`:
     - Checks `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GROQ_API_KEY`, `DATABASE_URL`.
     - Warns if found during sovereign runs.
   - `check_ollama_connectivity(url: &str, timeout_ms: u64) -> DiagnosticCheck`:
     - Performs loopback validation; probes `/api/tags`; deserializes models list; reports available models or missing model warning.
   - `check_ledger_state(ledger_path: Option<&Path>) -> DiagnosticCheck`:
     - Resolves ledger path (default `.needle/ledger/audit_chain.jsonl`).
     - If not present: Clean state PASS (0 blocks).
     - If present: Validates lines JSON, block sequence ($0, 1, \dots$), previous hash chain, and Ed25519 signature if Dalek is present. Reports block count or exact tamper sequence number.
   - `check_storage_integrity() -> DiagnosticCheck`:
     - Checks `.needle/` directory accessibility.

4. **CLI Entry Point Function**:
   ```rust
   pub async fn run(
       sovereign: bool,
       offline_strict: bool,
       ollama_url: &str,
       ledger_path: Option<&str>,
       json: bool,
   ) -> crate::Result<()> {
       let report = run_diagnostics(sovereign, offline_strict, ollama_url, ledger_path.map(Path::new))?;

       if json {
           println!("{}", serde_json::to_string_pretty(&report)?);
       } else {
           report.print_human();
       }

       if !report.is_success() {
           std::process::exit(1);
       }

       Ok(())
   }
   ```

---

## 6. Caveats

- **Ollama HTTP Client in Sovereign Mode**: Standard `reqwest` is gated under `cloud`. Raw TCP HTTP/1.1 implementation guarantees zero dependency on networking crates while retaining 100% loopback Ollama functionality.
- **Ledger Verification Dependency**: Full Ed25519 signature checks will hook directly into `src/ledger/verifier.rs` (developed in M4). For M1, a standalone SHA-256 and JSON sequence validator in `doctor.rs` provides full chain verification and seamlessly links with M4 modules.
- **File Touch Boundaries**: No changes to `src/embedding/`, `src/indexing/bm25.rs`, or `src/indexing/hnsw.rs`.

---

## 7. Conclusion

1. The exact CLI interface for `needle doctor --sovereign` is fully specified with options for `--sovereign`, `--offline-strict`, `--ollama-url`, `--ledger-path`, and `--json`.
2. All 5 core diagnostic checks (Feature flags, Cloud route elimination, Environment hygiene, Local Ollama connectivity with model inspection, and Cryptographic ledger integrity) are specified down to the struct, function signatures, wire protocol, and error paths.
3. Zero-panic guarantee is enforced throughout: zero `.unwrap()`, zero `.expect()`, zero `panic!()`.
4. Output formatting adheres to project conventions with checkmarks `[✓]`, `[✗]`, `[!]`, `[i]`, a diagnostics summary table, and standard exit codes (0, 1, 2).

---

## 8. Verification Method

Once implemented, the orchestrator and implementers can independently verify the subsystem via:

1. **Verify Default Build Preservation**:
   ```bash
   cargo build --release
   cargo run -- doctor
   # Doctor reports cloud mode active, Ollama connectivity, index status
   ```

2. **Verify Sovereign Build Compliance**:
   ```bash
   cargo build --release --no-default-features --features sovereign
   cargo run --no-default-features --features sovereign -- doctor --sovereign
   # All 7 checks pass with [✓], exit code 0
   ```

3. **Verify Zero Networking Crates in Sovereign Tree**:
   ```powershell
   cargo tree --no-default-features --features sovereign | Select-String "reqwest|hyper|sqlx|axum|tower-cookies|tower-http|open|urlencoding"
   # Output must be completely empty
   ```

4. **Verify Failure Diagnostics on Default Build**:
   ```bash
   cargo run -- doctor --sovereign
   # Outputs [✗] Compile-Time Mode: NON-SOVEREIGN, exits with code 1
   ```

5. **Verify Offline-Strict Rejection of Remote Hosts**:
   ```bash
   cargo run -- doctor --sovereign --offline-strict --ollama-url http://192.168.1.50:11434
   # Outputs [✗] Non-loopback host rejected under sovereign mode, exits with code 1
   ```

6. **Verify Clean Ledger State on Fresh Repository**:
   ```bash
   cargo run --no-default-features --features sovereign -- doctor --sovereign
   # Outputs [✓] Cryptographic Ledger: Chain intact (0 blocks, clean state)
   ```
