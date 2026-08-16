# IT Security Circular: Finance Ministry Tax Data Handling

**Effective Date:** January 1, 2026
**Scope:** All internal applications interacting with citizen tax data or authentication endpoints.

## 1. Authentication and Session Management

To prevent session hijacking and unauthorized persistent access, all digital authentication mechanisms must enforce strict session limits.

- **Token Expiration:** Authentication tokens (JWTs, session IDs, or API keys) issued to client applications **must expire within 15 minutes** of issuance. Infinite or 24-hour tokens are strictly prohibited for citizen-facing applications.

## 2. Data Privacy and Audit Logging

Audit logs are critical for forensic analysis, but they must not become a vector for data leaks.

- **PII Logging Restrictions:** Personally Identifiable Information (PII)—specifically including Social Security Numbers (SSN), Aadhaar numbers, tax IDs, and plaintext email addresses—**must not be logged in plaintext**. Any logging of these fields must be masked, hashed, or fully redacted prior to being written to application or system logs.

## 3. Cryptographic Standards

Legacy hashing algorithms have proven vulnerable to collision attacks and must be eradicated from the codebase.

- **Password Hashing:** All password hashes and cryptographic signatures must utilize **SHA-256 or stronger algorithms** (e.g., Argon2, bcrypt). The use of MD5 or SHA-1 is strictly prohibited under any circumstances.
