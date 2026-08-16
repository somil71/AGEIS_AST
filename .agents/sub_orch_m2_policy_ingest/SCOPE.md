# Scope: Milestone M2 - Policy Ingestion & Obligation Structuring

## Architecture
- `src/policy/mod.rs`: Public re-exports and module definition.
- `src/policy/clause.rs`:
  - `PolicyDocument`: Id, title, source_path, raw_text, format, clauses, metadata.
  - `PolicyClause`: Id, document_id, section_number, raw_text, obligations.
  - `PolicyObligation`: Id, clause_id, text, obligation_type, severity, target_entities, condition, action.
  - `ObligationType`: `Must`, `MustNot`, `Should`, `May`, `RequiredIf`, etc.
  - `Severity`: `Critical`, `High`, `Medium`, `Low`, `Informational`.
- `src/policy/parser.rs`:
  - Format support: `.pdf` (via `pdf-extract`), `.md`, `.txt`, `.policy`.
  - Scanned PDF check: If printable character count < 20, return explicit `Error::PolicyError` ("Scanned or unreadable PDF: insufficient extractable text").
  - Chunking / section extraction for clauses.
- `src/policy/structurer.rs`:
  - Extract obligations from raw clauses.
  - Support LLM extraction attempt (e.g. prompt template or local LLM interface if configured) with graceful and deterministic heuristic rule-based fallback.
  - Heuristic parser analyzes keywords ("shall", "must", "must not", "prohibited", "required", "should", "may", etc.) and pattern matching for severity and obligation types.
- `src/cli/policy.rs`:
  - `needle policy ingest <path>`: Ingest policy document, chunk into clauses, extract obligations, store in policy storage/ledger.
  - `needle policy list`: List ingested policies and summary of obligations.

## Feature Inventory
| # | Feature | Description | Milestone | Status |
|---|---------|-------------|-----------|--------|
| F7 | Policy Ingestion & Parsing | Ingest `.pdf`, `.md`, `.txt`, `.policy`, handle scanned PDF error (<20 chars) | M2 | IN_PROGRESS |
| F8 | Policy Clause Chunking | Split policies into structured `PolicyDocument`, `PolicyClause` | M2 | IN_PROGRESS |
| F9 | Obligation Structuring | Extract `PolicyObligation` with `ObligationType`, `Severity` via LLM/Heuristics | M2 | IN_PROGRESS |

## Interface Contracts
### `src/policy` ↔ `src/cli` & downstream engines
- `PolicyDocument::from_file(path: &Path) -> Result<PolicyDocument, Error>`
- `PolicyParser::parse(path: &Path, content_bytes: &[u8]) -> Result<PolicyDocument, Error>`
- `ObligationStructurer::structure_document(doc: &mut PolicyDocument) -> Result<Vec<PolicyObligation>, Error>`
- CLI commands integrate with `src/cli/policy.rs` and top-level CLI router.
