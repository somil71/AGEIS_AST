#!/usr/bin/env python3
from pathlib import Path
import re

content = Path("d:/AEGIS_AST/tests/e2e_sentinel_tests.rs").read_text(encoding="utf-8")
lines = content.splitlines()

# Let's inspect lines 400 to 650 and 700 to 1050
for i in range(400, 620):
    print(f"{i+1:4d}: {lines[i]}")
