# Milestone M4: CLI Integration, Error Handling, and Project Wiring Investigation & Design

## 1. Observation

Direct observations from examining the codebase, architecture documentation, and milestone requirements:

1. **Existing CLI Structure (`src/main.rs`, `src/cli/`)**:
   - `src/main.rs` lines 15-98: Subcommands (`Init`, `Search`, `Status`, `Reindex`, `Config`, `Bench`, `Watch`, `Mcp`, `Serve`, `Report`, `Graph`) are defined on `enum Commands` using `clap::Subcommand`.
   - `src/main.rs` lines 100-128: `main()` is an `async fn` returning `needle::Result<()>`. It initializes tracing (`needle=warn` default) and matches on `cli.command`, delegating to `cli::<module>::run(...)`.
   - `src/cli/mod.rs`: Exports subcommand modules (`bench`, `config`, `graph`, `init`, `mcp`, `reindex`, `report`, `search`, `serve`, `status`, `watch`).
   - `src/cli/` formatting patterns: Output uses `colored::Colorize` (`.bold()`, `.green()`, `.cyan()`, `.yellow()`, `.red()`, `.dimmed()`). Success messages use `✓` prefixes with green bold, errors use `eprintln!("{}: ...", "Error".red().bold())`, warnings use `yellow()`, and metrics use cyan/green values.

2. **Error Handling Architecture (`src/error.rs`)**:
   - `src/error.rs` lines 5-17: `pub enum Error` defines variants `Io`, `InvalidPath`, `IndexNotFound`, `ChunkingError`, `EmbeddingError`, `IndexError`, `QueryError`, `ConfigError`, `SerializationError`, `Other`.
   - `src/error.rs` lines 19-34: `fmt::Display` implementation formats errors with domain prefixes.
   - `src/error.rs` lines 38-60: `From` implementations for `std::io::Error`, `toml::de::Error`, `serde_json::Error`, `notify::Error`.
   - Currently lacks `LedgerError(String)` and `PolicyError(String)`.

3. **Storage & Workspace Layout (`src/storage/mod.rs`)**:
   - `Storage::default_index_dir()` lines 63-68: Resolves to `<project_root>/.needle/index` using `find_git_root()`.
   - `Storage::needle_dir()` lines 33-37: Resolves to `~/.needle/`.
   - Project-scoped `.needle/` directory is the canonical location for `.needle/ledger/audit_chain.jsonl`, `.needle/ledger/key.priv`, and `.needle/ledger/key.pub`.

4. **Module Layout (`src/lib.rs`)**:
   - `src/lib.rs` lines 6-20: Currently exports `analysis`, `chunking`, `config`, `embedding`, `error`, `graph`, `indexing`, `llm`, `query`, `schema`, `storage`, `server`, `watcher`, with `pub use error::{Error, Result};`.
   - Lacks `pub mod ledger;` export required for M4.

5. **Milestone M4 CLI Requirements**:
   - Subcommand: `needle ledger` with sub-actions:
     - `append`: `--report <path>` (required), `--type <entry_type>` (default `compliance_audit`), `--key <key_path>`, `--gen-key-if-missing`.
     - `verify`: `--ledger <path>`, `--verbose`.
     - `keygen`: `--output-dir <path>`, `--force`.
   - Zero `unwrap()`, `expect()`, or `panic!()` on user-input paths.
   - Strict private key redaction in logging/display.
   - Fresh/empty chain verification returns `Ok(0 blocks)` cleanly.
   - Tamper localization outputs exact broken sequence number.

---

## 2. Logic Chain

### 2.1 CLI Architecture & Command Wiring (`src/cli/ledger.rs` & `src/main.rs`)

1. **Subcommand Hierarchy**:
   `needle ledger` is structured as a top-level Clap subcommand with a nested `LedgerCommands` enum containing `Append`, `Verify`, and `Keygen`:

   ```rust
   // src/cli/ledger.rs
   use clap::Subcommand;

   #[derive(Subcommand, Debug, Clone)]
   pub enum LedgerCommands {
       /// Append an audit report or payload to the cryptographic ledger
       Append {
           /// Path to JSON report file
           #[arg(short, long, required = true)]
           report: String,

           /// Entry type: compliance_audit, security_scan, policy_ingest, codebase_snapshot, system_event
           #[arg(short, long, default_value = "compliance_audit")]
           r#type: String,

           /// Path to Ed25519 private key (defaults to .needle/ledger/key.priv)
           #[arg(short, long)]
           key: Option<String>,

           /// Automatically generate Ed25519 keypair if missing
           #[arg(long)]
           gen_key_if_missing: bool,
       },

       /// Verify the cryptographic integrity of the ledger chain
       Verify {
           /// Custom path to ledger file (default: .needle/ledger/audit_chain.jsonl)
           #[arg(short, long)]
           ledger: Option<String>,

           /// Show verification status of each individual block
           #[arg(short, long)]
           verbose: bool,
       },

       /// Generate a new Ed25519 keypair for ledger signing
       Keygen {
           /// Output directory for keypair (default: .needle/ledger/)
           #[arg(short, long)]
           output_dir: Option<String>,

           /// Overwrite existing keypair files
           #[arg(long)]
           force: bool,
       },
   }
   ```

2. **Integration in `src/main.rs`**:
   - Add `Ledger` to `enum Commands`:
     ```rust
     /// Cryptographic audit ledger commands
     Ledger {
         #[command(subcommand)]
         action: cli::ledger::LedgerCommands,
     },
     ```
   - Add match arm in `main()`:
     ```rust
     Commands::Ledger { action } => cli::ledger::run(action).await?,
     ```

3. **Subcommand Execution Logic**:

   - **`keygen` Execution Flow**:
     1. Determine output directory (`output_dir` or `default_ledger_dir()`).
     2. Create target directory via `std::fs::create_dir_all`.
     3. Define `priv_path = out_dir.join("key.priv")` and `pub_path = out_dir.join("key.pub")`.
     4. If either file exists and `!force`, return `Error::LedgerError("Key files already exist in ... Use --force to overwrite.")`.
     5. Call `LedgerKeypair::generate()`.
     6. Save keys to disk via `keypair.save_to_files(&priv_path, &pub_path)`.
     7. Print formatted success message displaying public key hex, masking private key.

   - **`append` Execution Flow**:
     1. Validate report file path existence. If missing, return `Error::InvalidPath`.
     2. Read file to string and parse with `serde_json::from_str::<serde_json::Value>()`. If parsing fails, return `Error::SerializationError`.
     3. Parse `EntryType` string into `EntryType` enum (`compliance_audit`, `security_scan`, `policy_ingest`, `codebase_snapshot`, `system_event`). If invalid, return `Error::LedgerError` with allowed list.
     4. Resolve private key path (`key` or `default_key_priv_path()`).
     5. If private key exists, load with `LedgerKeypair::load_from_file(&priv_key_path)`.
     6. If private key does not exist:
        - If `gen_key_if_missing` is true: generate keypair, save to `.needle/ledger/key.priv` and `.needle/ledger/key.pub`, log notification.
        - If `gen_key_if_missing` is false: return `Error::LedgerError("Private key not found at '...'. Run 'needle ledger keygen' or use '--gen-key-if-missing'.")`.
     7. Append block via `needle::ledger::append_to_ledger(&ledger_path, &keypair, entry_type, payload)`.
     8. Print formatted success card: Sequence #, Block Hash, Payload Hash, Entry Type, Signer Public Key, Timestamp, Ledger File.

   - **`verify` Execution Flow**:
     1. Resolve ledger path (`ledger` or `default_ledger_path()`).
     2. If file does not exist or has 0 bytes, print `✓ Ledger verified: 0 blocks (empty chain).` and return `Ok(())`.
     3. If blocks exist, call `needle::ledger::verify_ledger_file(&ledger_path)`.
     4. If verification succeeds:
        - If `verbose`: print block-by-block breakdown.
        - Print `✓ Ledger verified: {total_blocks} blocks valid. Chain integrity intact.` and latest block hash.
     5. If verification fails:
        - `verify_ledger_file` returns `Err(Error::LedgerError("TAMPER DETECTED at sequence {N}: {reason}"))`.
        - CLI logs formatted error and returns `Err(e)` which propagates to `main()` exiting with non-zero code.

---

### 2.2 Error Handling Subsystem (`src/error.rs`)

1. **Enum Extension**:
   ```rust
   // src/error.rs
   #[derive(Debug)]
   pub enum Error {
       Io(std::io::Error),
       InvalidPath(String),
       IndexNotFound(String),
       ChunkingError(String),
       EmbeddingError(String),
       IndexError(String),
       QueryError(String),
       ConfigError(String),
       SerializationError(String),
       LedgerError(String),    // M4: Ledger and cryptographic operations
       PolicyError(String),    // M2/M3: Policy parsing and compliance
       Other(Box<dyn std::error::Error>),
   }
   ```

2. **Display Formatting**:
   ```rust
   impl fmt::Display for Error {
       fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
           match self {
               Error::Io(e) => write!(f, "IO error: {}", e),
               Error::InvalidPath(p) => write!(f, "Invalid path: {}", p),
               Error::IndexNotFound(p) => write!(f, "Index not found at {}", p),
               Error::ChunkingError(e) => write!(f, "Chunking error: {}", e),
               Error::EmbeddingError(e) => write!(f, "Embedding error: {}", e),
               Error::IndexError(e) => write!(f, "Index error: {}", e),
               Error::QueryError(e) => write!(f, "Query error: {}", e),
               Error::ConfigError(e) => write!(f, "Config error: {}", e),
               Error::SerializationError(e) => write!(f, "Serialization error: {}", e),
               Error::LedgerError(e) => write!(f, "Ledger error: {}", e),
               Error::PolicyError(e) => write!(f, "Policy error: {}", e),
               Error::Other(e) => write!(f, "Error: {}", e),
           }
       }
   }
   ```

3. **Automatic Error Conversions**:
   ```rust
   impl From<ed25519_dalek::SignatureError> for Error {
       fn from(e: ed25519_dalek::SignatureError) -> Self {
           Error::LedgerError(format!("Ed25519 signature error: {e}"))
       }
   }

   impl From<hex::FromHexError> for Error {
       fn from(e: hex::FromHexError) -> Self {
           Error::LedgerError(format!("Hex decoding error: {e}"))
       }
   }
   ```

4. **Zero-Panic / Zero-Unwrap Rule Enforcement**:
   Every fallible operation on user inputs or filesystem paths is strictly converted to `Result<T, Error>`:

   | Operation | Potential Failure | Error Variant Handled |
   |---|---|---|
   | Read `--report <path>` | File not found, permission denied | `Error::InvalidPath` / `Error::Io` |
   | Parse report JSON | Corrupted JSON, syntax error | `Error::SerializationError` |
   | Parse `--type <str>` | Unrecognized string | `Error::LedgerError` |
   | Read `key.priv` | Missing key, bad hex, wrong length | `Error::LedgerError` / `Error::Io` |
   | Deserialize block line | Corrupted JSON in `.jsonl` | `Error::LedgerError("TAMPER DETECTED at sequence {N}...")` |
   | Validate sequence gap | Non-consecutive sequence # | `Error::LedgerError("TAMPER DETECTED at sequence {N}...")` |
   | Validate block hash | Payload or header altered | `Error::LedgerError("TAMPER DETECTED at sequence {N}...")` |
   | Validate Ed25519 sig | Invalid signature bytes / key | `Error::LedgerError("TAMPER DETECTED at sequence {N}...")` |

---

### 2.3 Module Exports & Cross-Subsystem Wiring

1. **`src/lib.rs`**:
   Add `pub mod ledger;`:
   ```rust
   pub mod analysis;
   pub mod chunking;
   pub mod config;
   pub mod embedding;
   pub mod error;
   pub mod graph;
   pub mod indexing;
   pub mod ledger; // <--- M4 export
   pub mod llm;
   pub mod query;
   pub mod schema;
   pub mod storage;
   pub mod server;
   pub mod watcher;

   pub use error::{Error, Result};
   ```

2. **`src/cli/mod.rs`**:
   Add `pub mod ledger;`:
   ```rust
   pub mod bench;
   pub mod config;
   pub mod graph;
   pub mod init;
   pub mod ledger; // <--- M4 CLI export
   pub mod mcp;
   pub mod reindex;
   pub mod report;
   pub mod search;
   pub mod serve;
   pub mod status;
   pub mod watch;
   ```

3. **`src/ledger/mod.rs`**:
   Root ledger module re-exporting key types and public functions:
   ```rust
   pub mod block;
   pub mod crypto;
   pub mod keypair;
   pub mod verifier;

   pub use block::{EntryType, LedgerBlock};
   pub use crypto::{sha256_hex, sign_ed25519, verify_ed25519_signature};
   pub use keypair::LedgerKeypair;
   pub use verifier::{verify_ledger_file, VerificationSummary, GENESIS_PREV_HASH};

   use crate::{Error, Result};
   use std::path::{Path, PathBuf};

   /// Returns `<project_root>/.needle/ledger`
   pub fn default_ledger_dir() -> PathBuf {
       crate::storage::Storage::default_index_dir()
           .parent()
           .unwrap_or(&PathBuf::from(".needle"))
           .join("ledger")
   }

   /// Returns `<project_root>/.needle/ledger/audit_chain.jsonl`
   pub fn default_ledger_path() -> PathBuf {
       default_ledger_dir().join("audit_chain.jsonl")
   }

   /// Returns `<project_root>/.needle/ledger/key.priv`
   pub fn default_key_priv_path() -> PathBuf {
       default_ledger_dir().join("key.priv")
   }

   /// Returns `<project_root>/.needle/ledger/key.pub`
   pub fn default_key_pub_path() -> PathBuf {
       default_ledger_dir().join("key.pub")
   }
   ```

4. **Integration with `needle audit --sign-ledger` (M3 Coupling)**:
   In `src/cli/audit.rs`:
   ```rust
   if sign_ledger {
       let ledger_path = needle::ledger::default_ledger_path();
       let priv_path = needle::ledger::default_key_priv_path();
       let pub_path = needle::ledger::default_key_pub_path();

       let keypair = needle::ledger::LedgerKeypair::load_or_generate(
           &priv_path,
           &pub_path,
           true, // generate if missing
       )?;

       let payload = serde_json::to_value(&compliance_report)?;
       let block = needle::ledger::append_to_ledger(
           &ledger_path,
           &keypair,
           needle::ledger::EntryType::ComplianceAudit,
           payload,
       )?;

       println!(
           "{}",
           format!("✓ Cryptographic ledger updated: Block #{} ({})", block.sequence, block.block_hash).green().bold()
       );
   }
   ```

---

## 3. Implementation Code Blueprint

### 3.1 Complete Proposed Implementation: `src/cli/ledger.rs`

```rust
//! `needle ledger [append|verify|keygen]` — Cryptographic audit trail management.

use clap::Subcommand;
use colored::Colorize;
use needle::ledger::{
    append_to_ledger, default_key_priv_path, default_key_pub_path, default_ledger_dir,
    default_ledger_path, verify_ledger_file, EntryType, LedgerKeypair,
};
use needle::{Error, Result};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Subcommand, Debug, Clone)]
pub enum LedgerCommands {
    /// Append an audit report or payload to the cryptographic ledger
    Append {
        /// Path to JSON report file
        #[arg(short, long, required = true)]
        report: String,

        /// Entry type: compliance_audit, security_scan, policy_ingest, codebase_snapshot, system_event
        #[arg(short, long, default_value = "compliance_audit")]
        r#type: String,

        /// Path to Ed25519 private key (defaults to .needle/ledger/key.priv)
        #[arg(short, long)]
        key: Option<String>,

        /// Automatically generate Ed25519 keypair if missing
        #[arg(long)]
        gen_key_if_missing: bool,
    },

    /// Verify the cryptographic integrity of the ledger chain
    Verify {
        /// Custom path to ledger file (default: .needle/ledger/audit_chain.jsonl)
        #[arg(short, long)]
        ledger: Option<String>,

        /// Show verification status of each individual block
        #[arg(short, long)]
        verbose: bool,
    },

    /// Generate a new Ed25519 keypair for ledger signing
    Keygen {
        /// Output directory for keypair (default: .needle/ledger/)
        #[arg(short, long)]
        output_dir: Option<String>,

        /// Overwrite existing keypair files
        #[arg(long)]
        force: bool,
    },
}

pub async fn run(action: LedgerCommands) -> Result<()> {
    match action {
        LedgerCommands::Keygen { output_dir, force } => run_keygen(output_dir, force),
        LedgerCommands::Append {
            report,
            r#type,
            key,
            gen_key_if_missing,
        } => run_append(report, r#type, key, gen_key_if_missing),
        LedgerCommands::Verify { ledger, verbose } => run_verify(ledger, verbose),
    }
}

fn run_keygen(output_dir: Option<String>, force: bool) -> Result<()> {
    let out_dir = match output_dir {
        Some(d) => PathBuf::from(d),
        None => default_ledger_dir(),
    };

    if !out_dir.exists() {
        std::fs::create_dir_all(&out_dir)?;
    }

    let priv_path = out_dir.join("key.priv");
    let pub_path = out_dir.join("key.pub");

    if (priv_path.exists() || pub_path.exists()) && !force {
        eprintln!(
            "{}: Key files already exist in {}.\n  Use --force to overwrite.",
            "Error".red().bold(),
            out_dir.display()
        );
        return Err(Error::LedgerError(format!(
            "Key files already exist at '{}'. Use --force to overwrite.",
            out_dir.display()
        )));
    }

    let keypair = LedgerKeypair::generate();
    keypair.save_to_files(&priv_path, &pub_path)?;

    println!("{}", "✓ Generated new Ed25519 keypair".green().bold());
    println!("  {}: {}", "Private key".bold(), priv_path.display().to_string().cyan());
    println!(
        "  {}: {} (pubkey: {})",
        "Public key".bold(),
        pub_path.display().to_string().cyan(),
        keypair.public_key_hex().dimmed()
    );

    Ok(())
}

fn run_append(
    report: String,
    r#type: String,
    key: Option<String>,
    gen_key_if_missing: bool,
) -> Result<()> {
    let report_path = PathBuf::from(&report);
    if !report_path.exists() {
        eprintln!(
            "{}: Report file not found at '{}'.",
            "Error".red().bold(),
            report_path.display()
        );
        return Err(Error::InvalidPath(format!(
            "Report file not found: {}",
            report_path.display()
        )));
    }

    let content = std::fs::read_to_string(&report_path)?;
    let payload: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        Error::SerializationError(format!(
            "Failed to parse report JSON at '{}': {e}",
            report_path.display()
        ))
    })?;

    let entry_type = EntryType::from_str(&r#type)?;

    let priv_key_path = match key {
        Some(k) => PathBuf::from(k),
        None => default_key_priv_path(),
    };
    let pub_key_path = default_key_pub_path();

    let keypair = if priv_key_path.exists() {
        LedgerKeypair::load_from_file(&priv_key_path)?
    } else if gen_key_if_missing {
        if let Some(parent) = priv_key_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let kp = LedgerKeypair::generate();
        kp.save_to_files(&priv_key_path, &pub_key_path)?;
        println!(
            "  {}",
            format!("(Generated missing keypair at {})", priv_key_path.display()).dimmed()
        );
        kp
    } else {
        eprintln!(
            "{}: Private key not found at '{}'.\n  Run `needle ledger keygen` or pass `--gen-key-if-missing`.",
            "Error".red().bold(),
            priv_key_path.display()
        );
        return Err(Error::LedgerError(format!(
            "Private key not found at '{}'. Run `needle ledger keygen` or pass `--gen-key-if-missing`.",
            priv_key_path.display()
        )));
    };

    let ledger_path = default_ledger_path();
    let block = append_to_ledger(&ledger_path, &keypair, entry_type, payload)?;

    println!(
        "{}",
        format!("✓ Block #{} appended to ledger", block.sequence).green().bold()
    );
    println!("  {}:   {}", "Block Hash".bold(), block.block_hash.cyan());
    println!("  {}: {}", "Payload Hash".bold(), block.payload_hash.dimmed());
    println!("  {}:  {:?}", "Entry Type".bold(), block.entry_type);
    println!("  {}:      {}", "Signer".bold(), block.signer_public_key.dimmed());
    println!("  {}:   {}", "Timestamp".bold(), block.timestamp.dimmed());
    println!("  {}:      {}", "Ledger File".bold(), ledger_path.display().to_string().dimmed());

    Ok(())
}

fn run_verify(ledger: Option<String>, verbose: bool) -> Result<()> {
    let ledger_path = match ledger {
        Some(l) => PathBuf::from(l),
        None => default_ledger_path(),
    };

    if !ledger_path.exists() {
        println!("{}", "✓ Ledger verified: 0 blocks (empty chain).".green().bold());
        println!("  {}: {}", "Ledger path".bold(), ledger_path.display().to_string().dimmed());
        return Ok(());
    }

    if verbose {
        println!("{}", "Verifying ledger blocks sequentially...".bold());
    }

    let summary = match verify_ledger_file(&ledger_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {e}", "Error".red().bold());
            return Err(e);
        }
    };

    if summary.total_blocks == 0 {
        println!("{}", "✓ Ledger verified: 0 blocks (empty chain).".green().bold());
    } else {
        println!(
            "{}",
            format!(
                "✓ Ledger verified: {} blocks valid. Chain integrity intact.",
                summary.total_blocks
            )
            .green()
            .bold()
        );
        if let Some(hash) = &summary.latest_block_hash {
            println!("  {}: {}", "Latest Block Hash".bold(), hash.cyan());
        }
    }
    println!("  {}: {}", "Ledger path".bold(), ledger_path.display().to_string().dimmed());

    Ok(())
}
```

---

## 4. Caveats

1. **Strict Key Material Redaction**:
   Even during error handling or CLI debug output, private key bytes or hex strings must never be included in error messages, formatted strings, or `eprintln!` output. All display implementations must use `"[REDACTED PRIVATE KEY]"` or display only the public key.

2. **Cross-Platform Path Separators**:
   Path handling in CLI flags and storage resolution must use `std::path::Path` and `PathBuf` rather than hardcoded `/` or `\\` strings to maintain Windows and Linux cross-platform compatibility.

3. **Concurrency on Ledger Append**:
   When multiple CLI instances append to `audit_chain.jsonl` concurrently, append operations should open the file in standard append mode (`std::fs::OpenOptions::new().create(true).append(true)`), which ensures atomic line appends on OS-level POSIX and Windows filesystems.

4. **Clap Subcommand Defaults**:
   Clap 4.4 parser handles default values via `#[arg(default_value = "...")]`. If custom enum parsing is desired directly in Clap derive, `#[derive(clap::ValueEnum)]` can be used on `EntryType` or mapped via `FromStr`.

---

## 5. Conclusion

1. **CLI Design**: `src/cli/ledger.rs` provides a clean, robust, and idiomatic interface matching the existing Needle CLI conventions, supporting `append`, `verify`, and `keygen`.
2. **Error Handling**: `src/error.rs` is expanded with `LedgerError(String)` and `PolicyError(String)` alongside automated `From` conversions, completely eliminating `unwrap()`, `expect()`, and `panic!()` on user-input paths.
3. **Module Wiring**: `src/lib.rs` and `src/cli/mod.rs` cleanly expose `pub mod ledger;` enabling direct cross-subsystem consumption by `needle audit --sign-ledger` and external callers.
4. **Tamper Localization & Fresh Chain**: Tampering produces exact-sequence error messages with non-zero exit codes, while fresh/empty chains verify cleanly returning 0 blocks and exit code 0.

---

## 6. Verification Method

To independently verify the CLI integration, error handling, and wiring:

1. **Keygen CLI Verification**:
   - Run: `needle ledger keygen --output-dir .needle/ledger --force`
   - **Expected Result**: Output prints `✓ Generated new Ed25519 keypair`, creates `key.priv` and `key.pub`, and displays only the public key.
   - Run again without `--force`: `needle ledger keygen --output-dir .needle/ledger`
   - **Expected Result**: Fails cleanly with exit code 1 and message: `Key files already exist at '...'. Use --force to overwrite.`

2. **Clean Fresh Chain Verification**:
   - Delete `.needle/ledger/audit_chain.jsonl` if present.
   - Run: `needle ledger verify`
   - **Expected Result**: Clean success (exit code 0) with message `✓ Ledger verified: 0 blocks (empty chain).`

3. **Append CLI Verification**:
   - Create a sample JSON file `test_report.json`: `{"audit": "pass", "score": 98}`
   - Run: `needle ledger append --report test_report.json --type compliance_audit --gen-key-if-missing`
   - **Expected Result**: Prints `✓ Block #0 appended to ledger` with `Block Hash`, `Payload Hash`, `Signer`, and `Timestamp`.

4. **Tamper Localization CLI Verification**:
   - Verify chain: `needle ledger verify` -> **Expected Result**: `✓ Ledger verified: 1 blocks valid. Chain integrity intact.`
   - Tamper with block 0 payload in `.needle/ledger/audit_chain.jsonl`.
   - Run: `needle ledger verify`
   - **Expected Result**: Exits with code 1 and outputs: `Error: TAMPER DETECTED at sequence 0: payload_hash mismatch...`

5. **Private Key Redaction Unit Test**:
   - Run unit test confirming `format!("{:?}", keypair)` contains `[REDACTED PRIVATE KEY]` and does not expose signing key bytes.
