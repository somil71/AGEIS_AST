## 2026-08-14T18:38:34Z
Worker assignment for Milestone M2 (Policy Ingestion & Obligation Structuring):
Implement Features F7, F8, F9:
1. Update `src/error.rs` to include `PolicyError(String)` variant and `Display` formatting.
2. Update `src/lib.rs` to export `pub mod policy;` (and `pub mod cli;` / `cli::policy` if appropriate).
3. Create `src/policy/mod.rs` with re-exports of clauses, parser, and structurer.
4. Create `src/policy/clause.rs`:
   - Data models: `PolicyDocument`, `PolicyClause`, `PolicyObligation`, `ObligationType` (`Must`, `MustNot`, `Should`, `May`, `RequiredIf`, `ProhibitedIf`), `Severity` (`Critical`, `High`, `Medium`, `Low`, `Informational`), and `PolicyFormat` (`Pdf`, `Markdown`, `PlainText`, `PolicyDsl`).
   - Implement helper methods (`total_obligations`, `all_obligations`, `is_mandatory`, `is_prohibition`, etc.), serde serialization/deserialization, and Display traits.
5. Create `src/policy/parser.rs`:
   - File ingestion supporting `.pdf` (using `pdf-extract`), `.md`, `.txt`, `.policy`.
   - Scanned-image PDF guard: If extracted printable character count < 20, return explicit `Error::PolicyError` indicating scanned/image-only PDF with no extractable text. Never silently create an empty document.
   - Clause chunking engine using hierarchical section/header detection (#/##/###, Section 1.1, Article 2, § 164.312, 1.1) with paragraph-based fallback (`\n\n`) and preamble preservation.
6. Create `src/policy/structurer.rs`:
   - `ObligationStructurer` implementing hybrid extraction:
     - LLM-assisted extraction using `LlmClient` with JSON schema prompt when available.
     - Deterministic heuristic rule fallback engine matching modal verbs ("must", "must not", "shall", "prohibited", "should", "may", "if ... must", etc.), extracting condition, action, target entities ("function", "endpoint", etc.), lexical keywords (BM25), semantic query, and calculating severity.
     - Support `--heuristic-only` / offline mode gracefully without network dependency.
7. Policy storage & CLI integration:
   - Ensure policies can be stored/loaded/listed at `.needle/policy/<policy_id>.json` (extend `Storage` in `src/storage/mod.rs` or provide helper in `src/policy/mod.rs`).
   - Implement `src/cli/policy.rs` for `needle policy ingest <path>` (with options: `--name`, `--version`, `--dry-run`, `--heuristic-only`, `--format`) and `needle policy list` (with `--format`, `--verbose`).
   - Integrate `Policy` subcommand into `src/main.rs` CLI parser and dispatcher.
8. Zero-Panic Rule: No unwrap(), expect(), or panic!() on user input paths. Use `?` error propagation.
9. Verification:
   - Write comprehensive unit and integration tests (including scanned PDF loud failure, format parsing, clause chunking, heuristic obligation extraction, and CLI operations).
   - Run `cargo check`, `cargo build`, and `cargo test` using the `run_command` tool. Ensure all tests pass.
