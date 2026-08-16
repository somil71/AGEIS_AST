#!/usr/bin/env python3
"""
Dump tests across different tiers to document forensic evidence.
"""
from pathlib import Path
import re

content = Path("d:/AEGIS_AST/tests/e2e_sentinel_tests.rs").read_text(encoding="utf-8")
lines = content.splitlines()

# Let's write a dumper for specific features and tiers
def print_test_range(start, end):
    for i in range(start-1, min(len(lines), end)):
        print(f"{i+1:4d}: {lines[i]}")

print("=== Tier 1 F16 & F17 (Lines 710-850) ===")
print_test_range(710, 850)

print("\n=== Tier 1 F18 & F19 & F20 (Lines 850-1020) ===")
print_test_range(850, 1020)

print("\n=== Tier 3 (Lines 1600-1740) ===")
print_test_range(1600, 1740)

print("\n=== Tier 4 (Lines 1750-1856) ===")
print_test_range(1750, 1856)
