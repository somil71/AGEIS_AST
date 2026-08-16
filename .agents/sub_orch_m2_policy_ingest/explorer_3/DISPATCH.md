## 2026-08-14T18:36:07Z
Task assignment received:
Investigate obligation structuring specifications for Feature F9 (Obligation Structuring & Heuristic Fallback) and CLI integration (`src/cli/policy.rs`).
Design data structures: PolicyDocument, PolicyClause, PolicyObligation, ObligationType, Severity.
Design structuring logic: LLM-based + heuristic rule fallback matching modal verbs ("shall", "must", "must not", etc.).
Design CLI commands `needle policy ingest <path>` and `needle policy list`.
Write findings to handoff.md and report to parent.
