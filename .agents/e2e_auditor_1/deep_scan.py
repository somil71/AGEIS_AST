#!/usr/bin/env python3
"""
Deep Forensic Scan of all 230 tests in e2e_sentinel_tests.rs
Categorize every test by what it actually executes vs facade checks.
"""
import re
from pathlib import Path

content = Path("d:/AEGIS_AST/tests/e2e_sentinel_tests.rs").read_text(encoding="utf-8")
lines = content.splitlines()

test_indices = [i for i, line in enumerate(lines) if "#[test]" in line]

facade_tests = []
file_exists_only_tests = []
string_check_only_tests = []
fixture_copy_only_tests = []
real_exec_tests = []

for idx in test_indices:
    # Function name
    fn_line_idx = idx + 1
    fn_name = None
    while fn_line_idx < len(lines) and fn_line_idx < idx + 5:
        m = re.search(r"(?:async\s+)?fn\s+([a-zA-Z0-9_]+)", lines[fn_line_idx])
        if m:
            fn_name = m.group(1)
            break
        fn_line_idx += 1

    # Extract body
    brace_depth = 0
    started = False
    body_lines = []
    for cur in range(fn_line_idx, len(lines)):
        l = lines[cur]
        body_lines.append(l)
        l_no_str = re.sub(r'"(?:\\.|[^"\\])*"', '', l)
        l_no_str = re.sub(r'r#".*?"#', '', l_no_str)
        l_no_str = re.sub(r'//.*$', '', l_no_str)
        brace_depth += l_no_str.count("{") - l_no_str.count("}")
        if "{" in l_no_str:
            started = True
        if started and brace_depth <= 0:
            break

    body = "\n".join(body_lines)
    
    # Classify
    # 1. Does it run CLI command or actual API?
    is_real_exec = False
    if "ctx.run_cmd" in body or "Command::new" in body:
        is_real_exec = True
    elif any(api in body for api in ["needle::", "parse_policy_file", "append_to_ledger", "verify_ledger_file", "LedgerKeypair::", "canonical_json", "sha256_hex", "evaluate_compliance", "structure_obligations", "LlmClient"]):
        is_real_exec = True

    if is_real_exec:
        real_exec_tests.append((fn_name, idx+1, body))
    else:
        # Check why it's a facade
        if "exists()" in body and not any(k in body for k in ["run_cmd", "verify", "parse", "append"]):
            file_exists_only_tests.append((fn_name, idx+1, body))
        elif 'url.contains' in body or 'content.contains' in body:
            string_check_only_tests.append((fn_name, idx+1, body))
        elif 'copy_fixture' in body and 'assert!(path.exists())' in body:
            fixture_copy_only_tests.append((fn_name, idx+1, body))
        else:
            facade_tests.append((fn_name, idx+1, body))

print(f"Total Tests: {len(test_indices)}")
print(f"Real Execution Tests: {len(real_exec_tests)}")
print(f"File Exists Only Tests: {len(file_exists_only_tests)}")
print(f"String/File Content Check Only Tests: {len(string_check_only_tests)}")
print(f"Fixture Copy Only Tests: {len(fixture_copy_only_tests)}")
print(f"Other Facade Tests: {len(facade_tests)}")
print(f"\nTotal Facade / Cheating Tests: {len(file_exists_only_tests) + len(string_check_only_tests) + len(fixture_copy_only_tests) + len(facade_tests)}")

print("\n--- Summary of ALL Real Execution Tests ---")
for fn, line_no, b in real_exec_tests:
    print(f"  Line {line_no}: {fn}")
