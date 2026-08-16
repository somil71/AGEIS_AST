#!/usr/bin/env python3
"""
Dump sample tests from e2e_sentinel_tests.rs
"""
from pathlib import Path
import re

content = Path("d:/AEGIS_AST/tests/e2e_sentinel_tests.rs").read_text(encoding="utf-8")
lines = content.splitlines()

# Let's inspect lines 140 to 600
for i in range(140, min(len(lines), 400)):
    print(f"{i+1:4d}: {lines[i]}")
