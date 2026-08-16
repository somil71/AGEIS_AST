#!/usr/bin/env python3
"""
Auditor Test Suite Parser & Static Verifier
"""
import re
from pathlib import Path

TESTS_FILE = Path("d:/AEGIS_AST/tests/e2e_sentinel_tests.rs")
content = TESTS_FILE.read_text(encoding="utf-8")
lines = content.splitlines()

# Extract all tests
test_blocks = []
current_module = "root"

mod_pattern = re.compile(r"^\s*(?:pub\s+)?mod\s+([a-zA-Z0-9_]+)\s*\{")
test_attr_pattern = re.compile(r"^\s*#\[test\]")
fn_pattern = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(")

module_stack = []

i = 0
while i < len(lines):
    line = lines[i]
    
    # Check mod declaration
    mm = mod_pattern.search(line)
    if mm:
        module_stack.append(mm.group(1))
    
    # Check end of mod
    # Simplistic: let's track module by lines
    
    if test_attr_pattern.search(line):
        # find the function name
        j = i + 1
        fn_name = None
        while j < len(lines) and j < i + 5:
            fm = fn_pattern.search(lines[j])
            if fm:
                fn_name = fm.group(1)
                break
            j += 1
        
        if fn_name:
            # extract body until end of fn
            k = j
            body_lines = []
            brace_count = 0
            started = False
            while k < len(lines):
                b_line = lines[k]
                body_lines.append(b_line)
                brace_count += b_line.count("{") - b_line.count("}")
                if "{" in b_line:
                    started = True
                if started and brace_count <= 0:
                    break
                k += 1
            
            test_blocks.append({
                "module": module_stack[-1] if module_stack else "root",
                "fn_name": fn_name,
                "start_line": i + 1,
                "end_line": k + 1,
                "body": "\n".join(body_lines)
            })
            i = k
    i += 1

print(f"Total test functions parsed: {len(test_blocks)}")
by_mod = {}
for t in test_blocks:
    mod = t["module"]
    by_mod.setdefault(mod, []).append(t["fn_name"])

for mod, funcs in by_mod.items():
    print(f"Module '{mod}': {len(funcs)} tests")

# Check for tautologies or trivial asserts
trivial_hits = []
for t in test_blocks:
    body = t["body"]
    
    # check for assert!(true), assert!(1 == 1), etc
    if re.search(r"assert!\s*\(\s*true\s*\)", body):
        trivial_hits.append((t["fn_name"], "assert!(true)"))
    if re.search(r"assert_eq!\s*\(\s*true\s*,\s*true\s*\)", body):
        trivial_hits.append((t["fn_name"], "assert_eq!(true, true)"))
    if re.search(r"assert_eq!\s*\(\s*1\s*,\s*1\s*\)", body):
        trivial_hits.append((t["fn_name"], "assert_eq!(1, 1)"))
    if re.search(r"assert_eq!\s*\(\s*0\s*,\s*0\s*\)", body):
        trivial_hits.append((t["fn_name"], "assert_eq!(0, 0)"))
    
    # check for empty body
    non_comment_lines = [l.strip() for l in body.splitlines() if l.strip() and not l.strip().startswith("//")]
    if len(non_comment_lines) <= 2:
        print(f"Short test: {t['fn_name']}: {body}")

print(f"Trivial assertion hits: {len(trivial_hits)}")
if trivial_hits:
    print(trivial_hits)
