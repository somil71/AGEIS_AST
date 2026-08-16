# Global Security Policy
## 1. No Hardcoded Secrets
API keys and passwords must not be hardcoded in the codebase.
## 2. SQL Injection Prevention
All database queries must use parameterized statements or ORM safely. String concatenation for SQL is strictly forbidden.
## 3. Cryptography
MD5 and SHA-1 are prohibited. Use SHA-256 or bcrypt.
