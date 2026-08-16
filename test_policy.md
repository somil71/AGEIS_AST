# Security Policy v1.0

## Section 1 — Authentication
1.1 The system MUST use bcrypt or argon2 for password hashing.
1.2 The system MUST NOT store passwords in plaintext.
1.3 Authentication tokens SHOULD expire after 24 hours.

## Section 2 — Data Retention
2.1 Audit logs MUST be retained for at least 7 years.
2.2 User data MUST be encrypted at rest using AES-256 or stronger.

## Section 3 — API Security
3.1 All API endpoints MUST validate input parameters.
3.2 SQL queries MUST use parameterized statements.
3.3 The system MUST NOT expose raw error messages to clients.
