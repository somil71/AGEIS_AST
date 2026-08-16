# Scope: Milestone M4 — Cryptographic Audit Ledger Subsystem

## Architecture
The Cryptographic Audit Ledger Subsystem provides an append-only, tamper-evident audit trail stored in `.needle/ledger/audit_chain.jsonl`. It ensures that every compliance audit, security scan, policy ingestion, or codebase snapshot is cryptographically hashed with SHA-256 and digitally signed using Ed25519.

```
+-------------------------------------------------------------------------------+
|                               CLI Subcommands                                 |
|   needle ledger append   |   needle ledger verify   |   needle ledger keygen  |
+------------------------------------+------------------------------------------+
                                     |
                                     v
+-------------------------------------------------------------------------------+
|                           Ledger Subsystem (src/ledger/)                      |
|                                                                               |
|  +------------------------+                     +--------------------------+  |
|  |       keypair.rs       |                     |        crypto.rs         |  |
|  | - LedgerKeypair        |                     | - SHA-256 block/payload  |  |
|  | - Strict Debug/Display |                     | - Ed25519 sign & verify  |  |
|  |   redaction            |                     |                          |  |
|  +-----------+------------+                     +-------------+------------+  |
|              |                                                |               |
|              +----------------------+-------------------------+               |
|                                     |                                         |
|                                     v                                         |
|  +-------------------------------------------------------------------------+  |
|  |                                block.rs                                 |  |
|  | - LedgerBlock, EntryType                                                |  |
|  | - Canonical JSON serialization for deterministic hashing                |  |
|  | - signing_preimage & block_preimage                                     |  |
|  +----------------------------------+--------------------------------------+  |
|                                     |                                         |
|              +----------------------+-------------------------+               |
|              |                                                |               |
|              v                                                v               |
|  +------------------------+                     +--------------------------+  |
|  |         mod.rs         |                     |       verifier.rs        |  |
|  | - append_to_ledger     |                     | - verify_ledger_file     |  |
|  | - Append-only file     |                     | - Clean empty chain      |  |
|  |   storage (.jsonl)     |                     | - Tamper localization    |  |
|  +------------------------+                     +--------------------------+  |
+-------------------------------------------------------------------------------+
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| F14 | Canonical JSON Block Encoding | Deterministic JSON serialization for block payloads and hashing | M4 | Spec Miner / R4 |
| F15 | SHA-256 Block & Payload Hashing | Cryptographic hash chaining using `sha2` (payload hash, signing preimage, block hash) | M4 | Spec Miner / R4 |
| F16 | Ed25519 Digital Signatures | Non-repudiable signing and verification via `ed25519-dalek` | M4 | Spec Miner / R4 |
| F17 | Redacted Keypair Security | Keypair management in `src/ledger/keypair.rs` with custom Debug/Display masking private keys (`"[REDACTED PRIVATE KEY]"`) | M4 | Spec Miner / R4 |
| F18 | Append-Only JSONL Writer | Append audit reports to `.needle/ledger/audit_chain.jsonl` with CLI `needle ledger append` | M4 | Spec Miner / R4 |
| F19 | Fresh/Empty Chain Clean Verification | CLI `needle ledger verify` cleanly verifies empty or non-existent ledger returning 0 blocks | M4 | Spec Miner / R4 |
| F20 | Tamper Detection & Localization | Verifier catches corrupted payloads, broken sequence, modified hashes, outputting exact broken sequence number | M4 | Spec Miner / R4 |

## Interface Contracts

### `src/ledger/` Public API
- `append_to_ledger(ledger_path: &Path, keypair: &LedgerKeypair, entry_type: EntryType, payload: serde_json::Value) -> Result<LedgerBlock, Error>`
- `verify_ledger_file(ledger_path: &Path) -> Result<VerificationSummary, Error>`
- `LedgerKeypair::generate() -> Self`
- `LedgerKeypair::from_bytes(bytes: &[u8; 32]) -> Self`
- `LedgerKeypair::public_key_hex(&self) -> String`
- `LedgerKeypair::load_or_generate(priv_path: &Path, pub_path: &Path, generate_if_missing: bool) -> Result<Self, Error>`
- Security: `fmt::Debug` and `fmt::Display` on `LedgerKeypair` NEVER emit secret key material; Debug prints `signing_key: "[REDACTED PRIVATE KEY]"`
- Types: `LedgerBlock`, `EntryType`, `VerificationSummary`
- Errors: `Error::LedgerError(String)`

### Preimage & Hashing Specifications
1. `payload_hash` = SHA-256(canonical_json(block.payload)) in 64-char lowercase hex
2. `signing_preimage` = `format!("{}:{}:{}:{:?}:{}", sequence, timestamp, prev_hash, entry_type, payload_hash)`
3. `signature` = Ed25519 signature over `signing_preimage.as_bytes()` in 128-char lowercase hex
4. `block_preimage` = `format!("{}:{}:{}:{:?}:{}:{}:{}", sequence, timestamp, prev_hash, entry_type, payload_hash, signer_public_key, signature)`
5. `block_hash` = SHA-256(`block_preimage.as_bytes()`) in 64-char lowercase hex
6. Genesis block (`sequence == 0`): `prev_hash == "0000000000000000000000000000000000000000000000000000000000000000"` (64 zeroes).

### CLI Subcommands (`src/cli/ledger.rs` & `src/main.rs`)
- `needle ledger append --report <path> [--type <entry_type>] [--key <priv_key_path>] [--gen-key-if-missing]`
- `needle ledger verify [--ledger <path>] [--verbose]`
- `needle ledger keygen [--output-dir <dir>] [--force]`
