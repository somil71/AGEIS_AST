#!/usr/bin/env python3
from pathlib import Path

content = Path("d:/AEGIS_AST/tests/e2e_sentinel_tests.rs").read_text(encoding="utf-8")
lines = content.splitlines()

for i in range(1580, 1670):
    print(f"{i+1:4d}: {lines[i]}")
