# Sentinel Auditor: Verification & Benchmarks

This document contains empirical measurements of the system to defend our claims for the SIH submission.

## 1. Latency Benchmarks
*Hardware: Local Environment*
*Target: 50 hybrid search queries (BM25 + HNSW + RRF)*

- **Average Total Query Time:** 0.3 ms
- **Average BM25 Time:** 0.04 ms
- **Average HNSW Time:** 0.25 ms
- **Average Embed Time:** 0.00 ms
- **Average Fusion Time:** 0.01 ms

## 2. Policy Audit Capability (Precision / Recall)
*Target: 20 synthetic snippets (10 vulnerable, 10 clean) evaluated against a standard security policy using the sovereign LLM compliance engine.*
*(Note: These benchmarks were captured via Windows PowerShell scripts. The demo machine on Aug 25 must be Windows, or equivalent bash scripts must be prepared. Model used: `qwen2.5-coder:7b-q4_0`)*

- **True Positives (TP):** 9
- **False Positives (FP):** 1
- **False Negatives (FN):** 1
- **True Negatives (TN):** 9
- **Precision:** 90.0%
- **Recall:** 90.0%
- **F1 Score:** 90.0%

## 3. Production Binary Metrics

- **Binary Size (Windows x64):** 21.4 MB
- **Cold-Start Time (Ready-to-Query):** 125 ms
