#!/usr/bin/env python3
"""
Inspect all 230 tests in e2e_sentinel_tests.rs
"""
import re
from pathlib import Path

content = Path("d:/AEGIS_AST/tests/e2e_sentinel_tests.rs").read_text(encoding="utf-8")
lines = content.splitlines()

test_indices = [i for i, line in enumerate(lines) if "#[test]" in line]
print(f"Detected {len(test_indices)} tests.")

tests = []
for idx in test_indices:
    # Function definition is on next line(s)
    fn_line_idx = idx + 1
    fn_name = None
    while fn_line_idx < len(lines) and fn_line_idx < idx + 5:
        m = re.search(r"(?:async\s+)?fn\s+([a-zA-Z0-9_]+)", lines[fn_line_idx])
        if m:
            fn_name = m.group(1)
            break
        fn_line_idx += 1
    
    if not fn_name:
        print(f"WARNING: Could not find fn name after line {idx+1}: {lines[idx]}")
        continue

    # Determine which module this test belongs to by looking backward
    mod_name = "unknown"
    for back in range(idx, -1, -1):
        mm = re.search(r"pub\s+mod\s+([a-zA-Z0-9_]+)", lines[back])
        if mm:
            mod_name = mm.group(1)
            break

    # Body lines until next #[test] or end of module
    # Let's find end of function using brace counting starting at fn_line_idx
    brace_depth = 0
    started = False
    body_lines = []
    for cur in range(fn_line_idx, len(lines)):
        l = lines[cur]
        # Ignore braces inside string literals roughly
        # For full accuracy:
        body_lines.append(l)
        # remove string literals for brace counting
        l_no_str = re.sub(r'"(?:\\.|[^"\\])*"', '', l)
        l_no_str = re.sub(r'r#".*?"#', '', l_no_str)
        l_no_str = re.sub(r'//.*$', '', l_no_str)
        brace_depth += l_no_str.count("{") - l_no_str.count("}")
        if "{" in l_no_str:
            started = True
        if started and brace_depth <= 0:
            break

    tests.append({
        "fn_name": fn_name,
        "mod_name": mod_name,
        "line_num": idx + 1,
        "body": "\n".join(body_lines)
    })

print(f"Extracted {len(tests)} test functions.")
tier_counts = {}
for t in tests:
    tier_counts[t["mod_name"]] = tier_counts.get(t["mod_name"], 0) + 1

for m, c in tier_counts.items():
    print(f"  {m}: {c} tests")

# Now check each test for:
# 1. Tautological assertions: assert!(true), assert!(1 == 1), etc.
# 2. Real API / CLI execution calls
# 3. Meaningful asserts

tautologies = []
no_asserts = []
cmd_exec_count = 0
api_call_count = 0

for t in tests:
    body = t["body"]
    fn = t["fn_name"]

    # Check tautologies
    if re.search(r"assert!\s*\(\s*true\s*\)", body):
        tautologies.append((fn, "assert!(true)"))
    if re.search(r"assert_eq!\s*\(\s*1\s*,\s*1\s*\)", body):
        tautologies.append((fn, "assert_eq!(1, 1)"))
    if re.search(r"assert_eq!\s*\(\s*\"([^\"]+)\"\s*,\s*\"\1\"\s*\)", body):
        tautologies.append((fn, "literal assert_eq"))

    # Check assertions
    has_assert = any(w in body for w in ["assert!", "assert_eq!", "assert_ne!", "assert_success", "assert_failure", "panic!"])
    if not has_assert:
        no_asserts.append(fn)

    # Check execution type
    if "run_cmd" in body or "Command::new" in body:
        cmd_exec_count += 1
    if any(call in body for call in ["parse_policy_file", "append_to_ledger", "verify_ledger_file", "LedgerKeypair", "canonical_json", "sha256", "sign_preimage", "verify_signature", "evaluate_compliance", "structure_obligations", "LlmClient"]):
        api_call_count += 1

print("\n--- Integrity Audit Results ---")
print(f"Tautological assertions: {len(tautologies)}")
if tautologies:
    for item in tautologies:
        print(" ", item)

print(f"Tests with no assertions: {len(no_asserts)}")
if no_asserts:
    for item in no_asserts:
        print(" ", item)

print(f"Tests executing CLI commands (run_cmd/Command): {cmd_exec_count}")
print(f"Tests invoking public crate APIs directly: {api_call_count}")
print(f"Total tests with genuine execution: {cmd_exec_count + api_call_count} / {len(tests)}")
