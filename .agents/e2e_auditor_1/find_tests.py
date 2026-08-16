from pathlib import Path

content = Path("d:/AEGIS_AST/tests/e2e_sentinel_tests.rs").read_text(encoding="utf-8")
lines = content.splitlines()

test_lines = []
for i, line in enumerate(lines, 1):
    if "#[test]" in line:
        test_lines.append((i, line))

print(f"Total lines with #[test]: {len(test_lines)}")
print("First 5:", test_lines[:5])
print("Last 10:", test_lines[-10:])
