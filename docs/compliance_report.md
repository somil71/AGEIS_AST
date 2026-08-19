# NEEDLE-SENTINEL Compliance Audit Report

> Generated: 2026-08-14T19:31:22.311788200+00:00

## Summary

| Metric | Count |
|---|---|
| ✅ Satisfied | 12 |
| ❌ Violated  | 0 |
| ⚠️  No Evidence | 0 |
| Total Obligations | 12 |

---

## Policy: Test Security Policy (v1.0.0)

**Compliance Score: 100%** (12 satisfied / 12 total)

### ✅ Satisfied

- **[0.0]** Preamble → `D:\AEGIS_AST\src\ledger\block.rs:36`
- **[#]** Section 1 — Authentication → `D:\AEGIS_AST\src\policy\clause.rs:193`
- **[1.1]** The system MUST use bcrypt or argon2 for → `D:\AEGIS_AST\src\policy\structurer.rs:623`
- **[1.2]** The system MUST NOT store passwords in plaintext → `D:\AEGIS_AST\src\policy\clause.rs:48`
- **[1.3]** Authentication tokens SHOULD expire after 24 hours → `D:\AEGIS_AST\src\policy\structurer.rs:289`
- **[#]** Section 2 — Data Retention → `D:\AEGIS_AST\src\policy\clause.rs:48`
- **[2.1]** Audit logs MUST be retained for at least → `D:\AEGIS_AST\src\policy\clause.rs:63`
- **[2.2]** User data MUST be encrypted at rest using → `D:\AEGIS_AST\src\policy\parser.rs:522`
- **[#]** Section 3 — API Security → `D:\AEGIS_AST\src\cli\serve\mod.rs:178`
- **[3.1]** All API endpoints MUST validate input parameters → `D:\AEGIS_AST\src\cli\mcp\tools_graph.rs:64`
- **[3.2]** SQL queries MUST use parameterized statements → `D:\AEGIS_AST\src\policy\clause.rs:75`
- **[3.3]** The system MUST NOT expose raw error messages → `D:\AEGIS_AST\src\policy\clause.rs:48`

