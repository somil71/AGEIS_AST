# Sentinel Auditor — Local-first Code Search

> Index any codebase. Search it semantically. Map its call graph. Plug it into any AI tool. Everything offline.

[![MIT License](https://img.shields.io/badge/license-MIT-7C3AED)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built_with-Rust-orange)](https://www.rust-lang.org/)
[![Release](https://img.shields.io/github/v/release/somil71/NEEDLE?color=7C3AED)](https://github.com/somil71/NEEDLE/releases)

---

## What is Sentinel Auditor?

Sentinel Auditor is a local-first code search engine and **AI Policy Auditor** that runs entirely on your machine. No cloud, no API keys, no data leaving your system.

**Use Case Scenario: Finance Ministry Tax Processing**
Imagine a highly classified local repository managing tax transactions. A new developer commits code that accidentally logs plain-text Aadhaar numbers and uses an expiring session token that lasts 24 hours instead of 15 minutes. Sentinel Auditor ingests the internal IT Security Policy, scans the codebase without ever touching the internet, and flags these exact policy contradictions instantly.

- **Policy Auditing** — Cross-reference ingested PDF/MD security circulars against code behavior.
- **Hybrid search** — BM25 keyword + HNSW vector search fused via Reciprocal Rank Fusion, sub-50ms
- **Call graph** — live D3 force graph with endpoint detection and architectural analysis
- **MCP server** — 14 tools for Claude Code, Cursor, Windsurf, Copilot
- **Desktop app** — native window via Tauri, or run headless as a CLI / Docker container

---

## Live Demo Script

**Pre-demo Checklist:**
- [ ] Ensure `ollama serve` is running.
- [ ] Run `.\scripts\warmup_demo.ps1` to load the LLM into VRAM so the demo doesn't stall on cold start.

Follow this flow to demonstrate the core value of Sentinel Auditor in 3 minutes:
1. **Ingest Policy:** Run `sentinel policy ingest demo/finance_ministry_data_policy.md` to feed the engine a realistic security circular.
2. **Index Code:** Run `sentinel init demo/` to index the vulnerable tax processing codebase.
3. **Audit Code:** Run `sentinel audit`. The system will cross-reference the policy against the code and output a markdown report flagging the plain-text PII logging and the excessive 24-hour token expiration.
4. **Legacy Language Fallback:** Point out `demo/legacy_system.cobol`. Emphasize that unsupported or legacy syntax gracefully degrades to full-text indexing instead of failing, making ancient systems fully searchable.
5. **Graph Impact:** Open the desktop app (`sentinel serve`) and view the D3 call graph to visually map the "blast radius" of the vulnerable functions.
6. **Ask Questions:** Use the semantic search bar to ask "Where are tokens issued?" and get instant hybrid search results.

---

## Install

### Windows (Desktop App)

Download **[Sentinel Auditor_0.1.0_x64-setup.exe](https://github.com/somil71/NEEDLE/releases/download/v0.1.0/Sentinel Auditor_0.1.0_x64-setup.exe)** and run the installer. Sentinel Auditor appears in your Start Menu.

### VS Code Extension

Download **[sentinel-search-0.5.0.vsix](https://github.com/somil71/NEEDLE/releases/download/v0.1.0/sentinel-search-0.5.0.vsix)** and install via:
```
Extensions panel → ⋯ → Install from VSIX
```

### Build from Source

```bash
git clone https://github.com/somil71/NEEDLE
cd NEEDLE
cargo build --release
# Binary at: target/release/sentinel
```

---

## Quick Start (CLI)

```bash
# Index a project
sentinel init ~/code/my-project

# Open the web UI
sentinel serve
# → http://localhost:7700

# Search from terminal
sentinel search "authentication middleware"

# Start MCP server for AI tools
sentinel mcp
```

---

## MCP Integration

Connect Sentinel Auditor to any MCP-compatible AI tool:

```json
{
  "mcpServers": {
    "sentinel": {
      "command": "sentinel",
      "args": ["mcp"]
    }
  }
}
```

**Claude Code:** `claude mcp add sentinel sentinel mcp`

**Cursor / Windsurf:** add to `.cursor/mcp.json` or `.windsurf/mcp.json`

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `search_code` | Hybrid keyword + semantic search |
| `find_callers` | Who calls a given function? |
| `find_callees` | What does a function call? |
| `find_similar` | Semantically similar code chunks |
| `get_god_nodes` | Highest-degree symbols |
| `get_endpoints` | All detected HTTP routes |
| `get_communities` | Label-propagation clusters |
| `get_surprises` | Cross-community edges |
| `get_file_structure` | Directory/module tree |
| `get_stats` | Index summary |
| `explain` | LLM explanation of a symbol |
| `get_health_score` | Codebase health score breakdown (0–100) |
| `get_security_scan` | Scan for secrets, XSS, and SQL injection vulnerabilities |
| `blast_radius` | Affected files and impact risk if a file changes |

---

## Supported Languages

| Language | Chunking | Call Graph |
|----------|----------|------------|
| Rust | AST (functions, structs, impls, traits) | ✓ |
| Python | AST (functions, classes, methods) | ✓ |
| TypeScript / JavaScript | AST (functions, classes, arrow fns) | ✓ |
| Go | AST (functions, types, interfaces) | ✓ |
| Java | AST (classes, methods) | ✓ |
| C / C++ | AST (functions, structs) | ✓ |
| Markdown | Section-by-section prose | — |
| PDF | Text extraction + paragraph chunks | — |

---

## Cloud / Docker

```bash
docker build -t sentinel .
docker run -p 8080:8080 \
  -e GITHUB_CLIENT_ID=... \
  -e GITHUB_CLIENT_SECRET=... \
  -e SESSION_SECRET=... \
  -v sentinel_data:/data \
  sentinel
```

Cloud mode adds GitHub OAuth and multi-repo support. Deploy to Railway, Render, or any Docker host.

---

## Architecture

```
src/
├── main.rs              # CLI entry (clap)
├── lib.rs               # Library crate root
├── schema.rs            # Chunk, Language, NodeKind types
├── chunking/            # Tree-sitter AST + prose chunking
├── indexing/            # BM25 inverted index + HNSW graph
├── query/               # QueryEngine + Reciprocal Rank Fusion
├── embedding/           # Hash-projection 384-dim embeddings
├── graph/               # CodeGraph, communities, god nodes
├── storage/             # JSON index persistence
├── server/              # Axum HTTP server + API routes
├── watcher/             # File watcher (live reindex)
└── assets/ui.html       # Web UI (single-file SPA, embedded at compile time)

src-tauri/               # Tauri desktop app wrapper
sentinel-vscode/           # VS Code extension
```

---

## License

MIT — see [LICENSE](LICENSE)
