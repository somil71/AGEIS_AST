#!/usr/bin/env python3
"""
Forensic Auditor Static & Cryptographic Analysis Script
for NEEDLE-SENTINEL E2E Test Suite and Fixtures
"""

import os
import re
import sys
import json
import hashlib
from pathlib import Path
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey

WORKSPACE = Path("d:/AEGIS_AST")
TESTS_FILE = WORKSPACE / "tests" / "e2e_sentinel_tests.rs"
FIXTURES_DIR = WORKSPACE / "tests" / "fixtures"

report = {
    "test_suite_stats": {},
    "suspicious_patterns": [],
    "fixture_analysis": {},
    "crypto_verification": {},
    "verdict_notes": []
}

def analyze_test_file():
    content = TESTS_FILE.read_text(encoding="utf-8")
    lines = content.splitlines()

    # Find all test functions
    test_funcs = []
    current_test = None
    current_tier = "top"
    
    tier_re = re.compile(r"pub\s+mod\s+(tier\d_[a-zA-Z0-9_]+)")
    test_attr_re = re.compile(r"^\s*#\[test\]")
    fn_re = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(")

    tests_by_tier = {}
    test_bodies = {}

    in_test = False
    current_test_name = None
    brace_depth = 0
    current_body = []

    for i, line in enumerate(lines, 1):
        tm = tier_re.search(line)
        if tm:
            current_tier = tm.group(1)
            tests_by_tier.setdefault(current_tier, [])

        if test_attr_re.search(line):
            in_test = True
            continue

        if in_test and not current_test_name:
            fm = fn_re.search(line)
            if fm:
                current_test_name = fm.group(1)
                tests_by_tier.setdefault(current_tier, []).append(current_test_name)
                test_funcs.append((current_tier, current_test_name, i))
                brace_depth = line.count("{") - line.count("}")
                current_body = [line]
                if brace_depth == 0 and "{" in line:
                    test_bodies[current_test_name] = "\n".join(current_body)
                    in_test = False
                    current_test_name = None
                continue

        if in_test and current_test_name:
            current_body.append(line)
            brace_depth += line.count("{") - line.count("}")
            if brace_depth <= 0:
                test_bodies[current_test_name] = "\n".join(current_body)
                in_test = False
                current_test_name = None

    report["test_suite_stats"]["total_tests_detected"] = len(test_funcs)
    report["test_suite_stats"]["tier_breakdown"] = {t: len(funcs) for t, funcs in tests_by_tier.items()}

    # Check for empty test bodies, trivial assertions, ignored tests
    ignored_tests = [line for line in lines if "#[ignore]" in line]
    report["test_suite_stats"]["ignored_tests_count"] = len(ignored_tests)

    trivial_asserts = []
    no_assert_tests = []
    short_tests = []

    trivial_patterns = [
        re.compile(r"assert!\s*\(\s*true\s*\)"),
        re.compile(r"assert!\s*\(\s*!false\s*\)"),
        re.compile(r"assert_eq!\s*\(\s*1\s*,\s*1\s*\)"),
        re.compile(r"assert_eq!\s*\(\s*true\s*,\s*true\s*\)"),
        re.compile(r"assert_eq!\s*\(\s*\"[^\"]*\"\s*,\s*\"[^\"]*\"\s*\)"), # literal eq
        re.compile(r"assert_ne!\s*\(\s*1\s*,\s*2\s*\)"),
    ]

    for name, body in test_bodies.items():
        # Check trivial assertions
        for pat in trivial_patterns:
            matches = pat.findall(body)
            if matches:
                # check if literal eq is comparing identical strings
                if "assert_eq!" in pat.pattern:
                    m = re.search(r"assert_eq!\s*\(\s*\"([^\"]*)\"\s*,\s*\"([^\"]*)\"\s*\)", body)
                    if m and m.group(1) == m.group(2):
                        trivial_asserts.append((name, m.group(0)))
                else:
                    trivial_asserts.append((name, matches))

        # Check if test has any assertion or verification
        has_assert = any(kw in body for kw in ["assert", "assert_eq", "assert_ne", "assert_success", "assert_failure", "panic!", "unwrap()", "expect("])
        if not has_assert:
            no_assert_tests.append(name)

        # Check body length
        body_lines = [l.strip() for l in body.splitlines() if l.strip() and not l.strip().startswith("//")]
        if len(body_lines) <= 2:
            short_tests.append((name, body))

    report["suspicious_patterns"] = {
        "trivial_asserts": trivial_asserts,
        "no_assert_tests": no_assert_tests,
        "short_tests": short_tests,
        "ignored_tests": ignored_tests
    }

def verify_crypto_and_fixtures():
    # 1. Keys verification
    keys_dir = FIXTURES_DIR / "keys"
    key_findings = {}

    for priv_name in ["test_auditor_ed25519", "secondary_auditor"]:
        priv_path = keys_dir / f"{priv_name}.priv"
        pub_path = keys_dir / f"{priv_name}.pub"

        if not priv_path.exists() or not pub_path.exists():
            key_findings[priv_name] = "MISSING FILE"
            continue

        priv_hex = priv_path.read_text(encoding="utf-8").strip()
        pub_hex = pub_path.read_text(encoding="utf-8").strip()

        try:
            priv_bytes = bytes.fromhex(priv_hex)
            assert len(priv_bytes) == 32, f"Expected 32 bytes, got {len(priv_bytes)}"
            key = Ed25519PrivateKey.from_private_bytes(priv_bytes)
            computed_pub_bytes = key.public_key().public_bytes_raw()
            computed_pub_hex = computed_pub_bytes.hex()

            is_match = (computed_pub_hex == pub_hex)
            key_findings[priv_name] = {
                "priv_bytes_len": len(priv_bytes),
                "pub_hex_len": len(pub_hex),
                "pub_matches_computed": is_match,
                "computed_pub_hex": computed_pub_hex,
                "recorded_pub_hex": pub_hex
            }
        except Exception as e:
            key_findings[priv_name] = f"ERROR: {e}"

    # Corrupted key check
    corrupt_path = keys_dir / "corrupted_key.priv"
    if corrupt_path.exists():
        c_hex = corrupt_path.read_text(encoding="utf-8").strip()
        key_findings["corrupted_key"] = {
            "exists": True,
            "content": c_hex,
            "len": len(c_hex),
            "is_truncated": len(bytes.fromhex(c_hex)) < 32
        }

    report["crypto_verification"]["keys"] = key_findings

    # 2. Ledgers verification
    ledgers_dir = FIXTURES_DIR / "ledgers"
    ledger_findings = {}

    # Check empty_chain.jsonl
    empty_path = ledgers_dir / "empty_chain.jsonl"
    ledger_findings["empty_chain"] = {
        "exists": empty_path.exists(),
        "size": empty_path.stat().st_size if empty_path.exists() else -1
    }

    # Verify valid_three_block_chain.jsonl
    valid_chain_path = ledgers_dir / "valid_three_block_chain.jsonl"
    if valid_chain_path.exists():
        lines = [l.strip() for l in valid_chain_path.read_text(encoding="utf-8").splitlines() if l.strip()]
        blocks = [json.loads(l) for l in lines]

        chain_verif = []
        expected_prev = "0000000000000000000000000000000000000000000000000000000000000000"

        for idx, block in enumerate(blocks):
            b_seq = block["sequence"]
            b_ts = block["timestamp"]
            b_prev = block["prev_hash"]
            b_type = block["entry_type"] # e.g. "policy_ingest"
            b_phash = block["payload_hash"]
            b_payload = block["payload"]
            b_pub = block["signer_public_key"]
            b_sig = block["signature"]
            b_hash = block["block_hash"]

            # Map entry_type to Debug representation matching Rust enum
            type_map = {
                "policy_ingest": "PolicyIngest",
                "compliance_audit": "ComplianceAudit",
                "codebase_snapshot": "CodebaseSnapshot"
            }
            debug_type = type_map.get(b_type, b_type)

            # Canonical JSON payload
            can_json = json.dumps(b_payload, sort_keys=True, separators=(',', ':')).encode('utf-8')
            computed_phash = hashlib.sha256(can_json).hexdigest()
            phash_valid = (computed_phash == b_phash)

            # Sequence check
            seq_valid = (b_seq == idx)

            # Prev hash check
            prev_valid = (b_prev == expected_prev)

            # Signature check
            signing_preimage = f"{b_seq}:{b_ts}:{b_prev}:{debug_type}:{b_phash}".encode('utf-8')
            try:
                pub_key_obj = Ed25519PublicKey.from_public_bytes(bytes.fromhex(b_pub))
                pub_key_obj.verify(bytes.fromhex(b_sig), signing_preimage)
                sig_valid = True
            except Exception as e:
                sig_valid = False

            # Block hash check
            block_preimage = f"{b_seq}:{b_ts}:{b_prev}:{debug_type}:{b_phash}:{b_pub}:{b_sig}".encode('utf-8')
            computed_bhash = hashlib.sha256(block_preimage).hexdigest()
            bhash_valid = (computed_bhash == b_hash)

            chain_verif.append({
                "sequence": b_seq,
                "phash_valid": phash_valid,
                "seq_valid": seq_valid,
                "prev_valid": prev_valid,
                "sig_valid": sig_valid,
                "bhash_valid": bhash_valid,
                "all_valid": all([phash_valid, seq_valid, prev_valid, sig_valid, bhash_valid])
            })

            expected_prev = b_hash

        ledger_findings["valid_three_block_chain"] = {
            "total_blocks": len(blocks),
            "blocks_verification": chain_verif,
            "is_perfect_chain": all(b["all_valid"] for b in chain_verif)
        }

    # Verify tampered files
    for t_name in ["tampered_payload_seq1", "tampered_sequence_gap", "tampered_prev_hash", "tampered_signature", "tampered_deleted_block"]:
        t_path = ledgers_dir / f"{t_name}.jsonl"
        if t_path.exists():
            t_lines = [l.strip() for l in t_path.read_text(encoding="utf-8").splitlines() if l.strip()]
            ledger_findings[t_name] = {
                "line_count": len(t_lines),
                "is_non_empty": len(t_lines) > 0
            }

    report["crypto_verification"]["ledgers"] = ledger_findings

    # 3. Policy PDF verification
    pol_dir = FIXTURES_DIR / "policies"
    pdf_findings = {}

    for pdf_name in ["valid_nist_cybersecurity.pdf", "scanned_image_only.pdf"]:
        p_path = pol_dir / pdf_name
        if p_path.exists():
            data = p_path.read_bytes()
            has_pdf_header = data.startswith(b"%PDF-")
            has_eof = b"%%EOF" in data
            has_text_stream = b"BT" in data and b"ET" in data
            has_xobject = b"/XObject" in data or b"/Image" in data
            pdf_findings[pdf_name] = {
                "size_bytes": len(data),
                "valid_pdf_structure": has_pdf_header and has_eof,
                "has_text_stream": has_text_stream,
                "has_image_xobject": has_xobject
            }
    report["fixture_analysis"]["pdfs"] = pdf_findings

    # 4. Sample codebase check
    sc_dir = FIXTURES_DIR / "sample_codebase"
    src_files = list((sc_dir / "src").glob("*.rs"))
    report["fixture_analysis"]["sample_codebase"] = {
        "cargo_toml_exists": (sc_dir / "Cargo.toml").exists(),
        "rust_files": [f.name for f in src_files],
        "rust_file_count": len(src_files)
    }

if __name__ == "__main__":
    analyze_test_file()
    verify_crypto_and_fixtures()
    print(json.dumps(report, indent=2))
