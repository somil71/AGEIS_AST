import os
import subprocess
import json
import shutil
import time

FIXTURES_DIR = r"D:\AEGIS_AST\tests\fixtures\precision_recall"
INDEX_DIR = os.path.join(FIXTURES_DIR, ".needle")
POLICY_DIR = os.path.join(INDEX_DIR, "policy")

# Ensure clean directory
if os.path.exists(FIXTURES_DIR):
    shutil.rmtree(FIXTURES_DIR)
os.makedirs(FIXTURES_DIR)

# 1. Write Policy
policy_path = os.path.join(FIXTURES_DIR, "security_policy.md")
with open(policy_path, "w", encoding="utf-8") as f:
    f.write("""# Global Security Policy
## 1. No Hardcoded Secrets
API keys and passwords must not be hardcoded in the codebase.
## 2. SQL Injection Prevention
All database queries must use parameterized statements or ORM safely. String concatenation for SQL is strictly forbidden.
## 3. Cryptography
MD5 and SHA-1 are prohibited. Use SHA-256 or bcrypt.
""")

# 2. Write Snippets (Vulnerable vs Clean)
snippets = {
    # TRUE POSITIVES (Vulnerable - should be flagged)
    "vuln_sql_1.py": 'query = f"SELECT * FROM users WHERE username = \'{user_input}\'"\ndb.execute(query)',
    "vuln_sql_2.ts": 'const sql = "UPDATE users SET age = " + age + " WHERE id = " + id;\ndb.query(sql);',
    "vuln_sql_3.go": 'q := "DELETE FROM logs WHERE user_id = " + req.URL.Query().Get("id")\ndb.Exec(q)',
    "vuln_secret_1.js": 'const AWS_SECRET_KEY = "AKIAIOSFODNN7EXAMPLE";\ns3.connect(AWS_SECRET_KEY);',
    "vuln_secret_2.rs": 'fn get_stripe_key() -> &\'static str {\n    "sk_live_51MabcdeFghijKLmnoP"\n}',
    "vuln_secret_3.py": 'def login():\n    db_password = "super_secret_db_password123!"',
    "vuln_crypto_1.java": 'MessageDigest md = MessageDigest.getInstance("MD5");\nmd.update(password.getBytes());',
    "vuln_crypto_2.py": 'import hashlib\nhashlib.sha1(b"Hello").hexdigest()',
    "vuln_crypto_3.c": 'EVP_DigestInit_ex(mdctx, EVP_md5(), NULL);',
    "vuln_crypto_4.ts": 'import crypto from "crypto";\nconst hash = crypto.createHash("md5").update(data).digest("hex");',
    
    # TRUE NEGATIVES (Clean - should NOT be flagged)
    "clean_sql_1.py": 'query = "SELECT * FROM users WHERE username = %s"\ndb.execute(query, (user_input,))',
    "clean_sql_2.ts": 'const sql = "UPDATE users SET age = ? WHERE id = ?";\ndb.query(sql, [age, id]);',
    "clean_sql_3.go": 'q := "DELETE FROM logs WHERE user_id = $1"\ndb.Exec(q, req.URL.Query().Get("id"))',
    "clean_secret_1.js": 'const AWS_SECRET_KEY = process.env.AWS_SECRET_KEY;\ns3.connect(AWS_SECRET_KEY);',
    "clean_secret_2.rs": 'fn get_stripe_key() -> String {\n    std::env::var("STRIPE_KEY").unwrap()\n}',
    "clean_secret_3.py": 'import os\ndef login():\n    db_password = os.getenv("DB_PASSWORD")',
    "clean_crypto_1.java": 'MessageDigest md = MessageDigest.getInstance("SHA-256");\nmd.update(password.getBytes());',
    "clean_crypto_2.py": 'import hashlib\nhashlib.sha256(b"Hello").hexdigest()',
    "clean_crypto_3.c": 'EVP_DigestInit_ex(mdctx, EVP_sha256(), NULL);',
    "clean_crypto_4.ts": 'import crypto from "crypto";\nconst hash = crypto.createHash("sha256").update(data).digest("hex");',
}

for name, content in snippets.items():
    with open(os.path.join(FIXTURES_DIR, name), "w", encoding="utf-8") as f:
        f.write(content)

# 3. Find binary
exe_path = r"D:\AEGIS_AST\target\release\sentinel.exe"
if not os.path.exists(exe_path):
    exe_path = r"D:\AEGIS_AST\target\release\needle.exe"

# 4. Ingest and Index
print("Ingesting policy...")
subprocess.run([exe_path, "policy", "ingest", "security_policy.md"], cwd=FIXTURES_DIR, check=True, capture_output=True)

print("Indexing snippets...")
subprocess.run([exe_path, "init", "."], cwd=FIXTURES_DIR, check=True, capture_output=True)

# 5. Audit
print("Running audit (JSON mode)...")
result = subprocess.run([exe_path, "audit", "--json"], cwd=FIXTURES_DIR, capture_output=True, text=True)

try:
    stdout = result.stdout
    start = stdout.find("[")
    end = stdout.rfind("]")
    if start != -1 and end != -1:
        json_str = stdout[start:end+1]
        reports = json.loads(json_str)
    else:
        raise ValueError("JSON array not found in output")
except Exception as e:
    print("Failed to parse audit JSON output.")
    print("STDOUT:", result.stdout)
    print("STDERR:", result.stderr)
    exit(1)

# 6. Compute Metrics
tp, fp, fn, tn = 0, 0, 0, 0

# Extract flagged files from report
flagged_files = set()
for r in reports:
    for link in r.get("links", []):
        status = link.get("status", {})
        if "Violated" in status:
            evidence_list = status["Violated"].get("conflicting", [])
            for ev in evidence_list:
                fname = os.path.basename(ev.get("file_path", ""))
                flagged_files.add(fname)

for fname in snippets:
    is_vuln = fname.startswith("vuln_")
    is_flagged = fname in flagged_files
    
    if is_vuln and is_flagged:
        tp += 1
    elif not is_vuln and is_flagged:
        fp += 1
    elif is_vuln and not is_flagged:
        fn += 1
    elif not is_vuln and not is_flagged:
        tn += 1

precision = tp / (tp + fp) if (tp + fp) > 0 else 0.0
recall = tp / (tp + fn) if (tp + fn) > 0 else 0.0
f1 = 2 * precision * recall / (precision + recall) if (precision + recall) > 0 else 0.0

print("\n===========================================================")
print("  PRECISION / RECALL EVALUATION")
print("===========================================================")
print(f"Total Snippets Evaluated: {len(snippets)}")
print(f"True Positives (TP) : {tp}")
print(f"False Positives (FP): {fp}")
print(f"False Negatives (FN): {fn}")
print(f"True Negatives (TN) : {tn}")
print("-----------------------------------")
print(f"Precision : {precision:.2%}")
print(f"Recall    : {recall:.2%}")
print(f"F1 Score  : {f1:.2%}")
print("===========================================================")
