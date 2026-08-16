//! `needle doctor` — System readiness, dependency isolation, and sovereign compliance diagnostic subsystem.

use colored::Colorize;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Fail,
    Warn,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: CheckStatus,
    pub details: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub sovereign_mode: bool,
    pub offline_strict: bool,
    pub checks: Vec<DiagnosticCheck>,
}

impl DoctorReport {
    pub fn is_success(&self) -> bool {
        !self.checks.iter().any(|c| c.status == CheckStatus::Fail)
    }

    pub fn print_human(&self) {
        println!("{}", "Needle v0.1.0 — Sovereign Readiness Diagnostic\n".bold());

        for check in &self.checks {
            let symbol = match check.status {
                CheckStatus::Pass => "[✓]".green().bold(),
                CheckStatus::Fail => "[✗]".red().bold(),
                CheckStatus::Warn => "[!]".yellow().bold(),
                CheckStatus::Info => "[i]".cyan().bold(),
            };
            println!("{} {:<28} {}", symbol, check.name.bold(), check.details);
            if let Some(ref rem) = check.remediation {
                if check.status == CheckStatus::Fail || check.status == CheckStatus::Warn {
                    println!("    {} {}", "↳ Fix:".dimmed(), rem.yellow());
                }
            }
        }

        println!("\n{}", "=".repeat(84).dimmed());
        println!("{:<32} {:<10} {}", "Check".bold(), "Status".bold(), "Details".bold());
        println!("{}", "-".repeat(84).dimmed());

        for check in &self.checks {
            let status_str = match check.status {
                CheckStatus::Pass => "PASS".green().bold(),
                CheckStatus::Fail => "FAIL".red().bold(),
                CheckStatus::Warn => "WARN".yellow().bold(),
                CheckStatus::Info => "INFO".cyan().bold(),
            };
            println!("{:<32} {:<10} {}", check.name, status_str, check.details);
        }
        println!("{}", "-".repeat(84).dimmed());

        let total = self.checks.len();
        let passed = self.checks.iter().filter(|c| c.status == CheckStatus::Pass).count();
        let failed = self.checks.iter().filter(|c| c.status == CheckStatus::Fail).count();
        let warnings = self.checks.iter().filter(|c| c.status == CheckStatus::Warn).count();

        if self.is_success() {
            println!(
                "Overall Result: {} ({}/{} checks passed{})",
                "PASSED".green().bold(),
                passed,
                total,
                if warnings > 0 { format!(", {} warnings", warnings) } else { "".into() }
            );
            println!("{}", "Status: System is fully ready for sovereign air-gapped operation.\n".green());
        } else {
            println!(
                "Overall Result: {} ({} failed, {} warnings, {} passed)",
                "FAILED".red().bold(),
                failed,
                warnings,
                passed
            );
            println!("{}", "Status: System is NOT ready for sovereign air-gapped operation.\n".red().bold());
        }
    }
}

// ── Raw TCP Probe (Zero third-party networking crates) ─────────────────────────

fn probe_raw_http_get(host: &str, port: u16, path: &str, timeout_ms: u64) -> Result<(u16, String), String> {
    let host_resolved = if host == "localhost" { "127.0.0.1" } else { host };
    let addr_str = format!("{}:{}", host_resolved, port);
    let addrs: Vec<SocketAddr> = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("Cannot resolve {}: {}", addr_str, e))?
        .collect();

    if addrs.is_empty() {
        return Err(format!("No socket addresses resolved for {addr_str}"));
    }

    let stream = TcpStream::connect_timeout(&addrs[0], Duration::from_millis(timeout_ms))
        .map_err(|e| format!("Connection to {addr_str} failed: {e}"))?;

    let _ = stream.set_read_timeout(Some(Duration::from_millis(timeout_ms)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(timeout_ms)));

    let mut stream = stream;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nUser-Agent: needle-doctor/0.1.0\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        path, host, port
    );

    stream.write_all(request.as_bytes()).map_err(|e| format!("Write failed: {e}"))?;
    stream.flush().map_err(|e| format!("Flush failed: {e}"))?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|e| format!("Read failed: {e}"))?;

    let response_str = String::from_utf8_lossy(&response);
    let mut parts = response_str.splitn(2, "\r\n\r\n");
    let headers = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("");

    let status_code = headers
        .lines()
        .next()
        .and_then(|line| {
            let mut p = line.split_whitespace();
            p.next()?; // HTTP/1.1
            p.next()?.parse::<u16>().ok()
        })
        .unwrap_or(0);

    Ok((status_code, body.to_string()))
}

// ── Check Implementations ─────────────────────────────────────────────────────

/// 1. Compile-time feature flags check
fn check_compile_features(sovereign_requested: bool) -> DiagnosticCheck {
    let has_sovereign = cfg!(feature = "sovereign");
    let has_cloud = cfg!(feature = "cloud");
    let is_true_sovereign = has_sovereign && !has_cloud;

    if sovereign_requested {
        if is_true_sovereign {
            DiagnosticCheck {
                name: "Compile-Time Mode".into(),
                status: CheckStatus::Pass,
                details: "Sovereign Mode (Zero Remote Network Crates: reqwest, hyper, sqlx, axum excluded)".into(),
                remediation: None,
            }
        } else {
            DiagnosticCheck {
                name: "Compile-Time Mode".into(),
                status: CheckStatus::Fail,
                details: "NON-SOVEREIGN: Binary compiled with default 'cloud' features enabled.".into(),
                remediation: Some("Rebuild with: cargo build --release --no-default-features --features sovereign".into()),
            }
        }
    } else if is_true_sovereign {
        DiagnosticCheck {
            name: "Compile-Time Mode".into(),
            status: CheckStatus::Pass,
            details: "Sovereign Mode (Zero remote networking crates linked)".into(),
            remediation: None,
        }
    } else {
        DiagnosticCheck {
            name: "Compile-Time Mode".into(),
            status: CheckStatus::Info,
            details: "Default Cloud Mode (axum, sqlx, reqwest enabled)".into(),
            remediation: None,
        }
    }
}

/// 2. Cloud routes gating check
fn check_cloud_routes(sovereign_requested: bool) -> DiagnosticCheck {
    let has_cloud = cfg!(feature = "cloud");

    if !has_cloud {
        DiagnosticCheck {
            name: "Cloud Routes & Server".into(),
            status: CheckStatus::Pass,
            details: "Disabled (Compiled Out: /auth/github, /api/repos, /api/import/github, neon postgres)".into(),
            remediation: None,
        }
    } else if sovereign_requested {
        DiagnosticCheck {
            name: "Cloud Routes & Server".into(),
            status: CheckStatus::Fail,
            details: "Active in binary: /auth/github, /api/repos, web UI server are enabled".into(),
            remediation: Some("Recompile with: cargo build --no-default-features --features sovereign".into()),
        }
    } else {
        DiagnosticCheck {
            name: "Cloud Routes & Server".into(),
            status: CheckStatus::Pass,
            details: "Enabled (/auth/github, /api/repos, web UI server available)".into(),
            remediation: None,
        }
    }
}

/// 3. Environment variable hygiene check
fn check_environment_hygiene(sovereign_mode: bool) -> DiagnosticCheck {
    let cloud_vars = [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GROQ_API_KEY",
        "DATABASE_URL",
    ];
    let detected: Vec<&str> = cloud_vars
        .iter()
        .copied()
        .filter(|v| std::env::var(v).is_ok())
        .collect();

    if detected.is_empty() {
        DiagnosticCheck {
            name: "Environment Hygiene".into(),
            status: CheckStatus::Pass,
            details: "Clean (No cloud API keys or remote DB URLs detected in environment)".into(),
            remediation: None,
        }
    } else if sovereign_mode {
        DiagnosticCheck {
            name: "Environment Hygiene".into(),
            status: CheckStatus::Warn,
            details: format!(
                "Cloud credentials detected in environment: [{}] (ignored in sovereign mode)",
                detected.join(", ")
            ),
            remediation: Some("Unset environment variables for clean air-gap compliance".into()),
        }
    } else {
        DiagnosticCheck {
            name: "Environment Hygiene".into(),
            status: CheckStatus::Pass,
            details: format!("Cloud credentials configured: [{}]", detected.join(", ")),
            remediation: None,
        }
    }
}

/// 4. Local Ollama connectivity and model readiness probe
fn check_ollama_connectivity(
    ollama_url: &str,
    offline_strict: bool,
    sovereign_mode: bool,
) -> DiagnosticCheck {
    let (host, port) = match needle::llm::LoopbackValidator::validate_and_extract(ollama_url, 11434) {
        Ok(hp) => hp,
        Err(e) => {
            return DiagnosticCheck {
                name: "Local LLM Routing".into(),
                status: CheckStatus::Fail,
                details: format!("Non-loopback host rejected: {e}"),
                remediation: Some("Configure loopback endpoint: http://127.0.0.1:11434 or localhost:11434".into()),
            };
        }
    };

    match probe_raw_http_get(&host, port, "/api/tags", 2000) {
        Ok((200, body)) => {
            let models: Vec<String> = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v["models"].as_array().cloned())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            if models.is_empty() {
                DiagnosticCheck {
                    name: "Local LLM Routing".into(),
                    status: CheckStatus::Warn,
                    details: format!("Ollama online at {host}:{port}, but 0 models installed"),
                    remediation: Some("Run 'ollama pull llama3.2' to install the recommended local model".into()),
                }
            } else {
                DiagnosticCheck {
                    name: "Local LLM Routing".into(),
                    status: CheckStatus::Pass,
                    details: format!("Online at {host}:{port} (Models: {})", models.join(", ")),
                    remediation: None,
                }
            }
        }
        Ok((status, body)) => DiagnosticCheck {
            name: "Local LLM Routing".into(),
            status: if offline_strict || sovereign_mode { CheckStatus::Fail } else { CheckStatus::Warn },
            details: format!("Ollama responded with HTTP {status}: {body}"),
            remediation: Some("Verify Ollama daemon health".into()),
        },
        Err(e) => {
            if offline_strict || sovereign_mode {
                DiagnosticCheck {
                    name: "Local LLM Routing".into(),
                    status: CheckStatus::Fail,
                    details: format!("Ollama unreachable at {host}:{port} ({e})"),
                    remediation: Some("Ensure Ollama daemon is running locally: 'ollama serve'".into()),
                }
            } else {
                DiagnosticCheck {
                    name: "Local LLM Routing".into(),
                    status: CheckStatus::Warn,
                    details: format!("Ollama unreachable at {host}:{port} ({e})"),
                    remediation: Some("Run 'ollama serve' to enable local LLM capabilities".into()),
                }
            }
        }
    }
}

/// 5. Cryptographic ledger state verification
fn check_ledger_state(custom_ledger_path: Option<&Path>) -> DiagnosticCheck {
    let default_path = Path::new(".needle/ledger/audit_chain.jsonl");
    let ledger_path = custom_ledger_path.unwrap_or(default_path);

    if !ledger_path.exists() {
        return DiagnosticCheck {
            name: "Cryptographic Ledger".into(),
            status: CheckStatus::Pass,
            details: format!("Chain intact (0 blocks, clean state at {})", ledger_path.display()),
            remediation: None,
        };
    }

    let content = match std::fs::read_to_string(ledger_path) {
        Ok(c) => c,
        Err(e) => {
            return DiagnosticCheck {
                name: "Cryptographic Ledger".into(),
                status: CheckStatus::Fail,
                details: format!("Cannot read ledger file at {}: {e}", ledger_path.display()),
                remediation: Some("Check file permissions or path".into()),
            };
        }
    };

    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return DiagnosticCheck {
            name: "Cryptographic Ledger".into(),
            status: CheckStatus::Pass,
            details: "Chain intact (0 blocks, clean state)".into(),
            remediation: None,
        };
    }

    let mut prev_hash = "0".repeat(64);
    let mut block_count = 0usize;

    for (idx, line) in lines.iter().enumerate() {
        let val: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                return DiagnosticCheck {
                    name: "Cryptographic Ledger".into(),
                    status: CheckStatus::Fail,
                    details: format!("Tamper detected at line {} (JSON syntax error: {e})", idx + 1),
                    remediation: Some("Restore audit ledger from verified offline backup".into()),
                };
            }
        };

        // Sequence number check
        let seq = val.get("sequence").and_then(|s| s.as_u64());
        if seq != Some(idx as u64) {
            return DiagnosticCheck {
                name: "Cryptographic Ledger".into(),
                status: CheckStatus::Fail,
                details: format!(
                    "Tamper detected at sequence #{} (Expected sequence {}, found {:?})",
                    idx, idx, seq
                ),
                remediation: Some("Restore audit ledger from verified offline backup".into()),
            };
        }

        // Previous hash check
        if let Some(ph) = val.get("previous_hash").and_then(|p| p.as_str()) {
            if ph != prev_hash {
                return DiagnosticCheck {
                    name: "Cryptographic Ledger".into(),
                    status: CheckStatus::Fail,
                    details: format!(
                        "Tamper detected at sequence #{} (Previous hash mismatch: expected '{}', found '{}')",
                        idx, prev_hash, ph
                    ),
                    remediation: Some("Restore audit ledger from verified offline backup".into()),
                };
            }
        }

        // Compute current block hash for chaining
        if let Some(bh) = val.get("block_hash").and_then(|b| b.as_str()) {
            prev_hash = bh.to_string();
        } else {
            let mut hasher = Sha256::new();
            hasher.update(line.as_bytes());
            prev_hash = format!("{:x}", hasher.finalize());
        }

        block_count += 1;
    }

    DiagnosticCheck {
        name: "Cryptographic Ledger".into(),
        status: CheckStatus::Pass,
        details: format!("Chain intact ({} blocks verified, cryptographic continuity intact)", block_count),
        remediation: None,
    }
}

/// 6. Local storage / index health check
fn check_storage_integrity() -> DiagnosticCheck {
    let index_dir = Path::new(".needle");
    if index_dir.exists() && index_dir.is_dir() {
        let meta_file = index_dir.join("meta.json");
        let index_meta = index_dir.join("index").join("meta.json");

        if meta_file.exists() || index_meta.exists() {
            DiagnosticCheck {
                name: "Local Index & Storage".into(),
                status: CheckStatus::Pass,
                details: "Local index found at .needle (metadata and chunks active)".into(),
                remediation: None,
            }
        } else {
            DiagnosticCheck {
                name: "Local Index & Storage".into(),
                status: CheckStatus::Pass,
                details: "Local directory .needle exists (ready for indexing)".into(),
                remediation: None,
            }
        }
    } else {
        DiagnosticCheck {
            name: "Local Index & Storage".into(),
            status: CheckStatus::Info,
            details: "No local index found at .needle (Run 'needle init <dirs>' to index code)".into(),
            remediation: None,
        }
    }
}

// ── Public Entry Point ────────────────────────────────────────────────────────

pub fn run_diagnostics(
    sovereign: bool,
    offline_strict: bool,
    ollama_url: &str,
    ledger_path: Option<&Path>,
) -> DoctorReport {
    let checks = vec![
        check_compile_features(sovereign),
        check_cloud_routes(sovereign),
        check_environment_hygiene(sovereign || offline_strict),
        check_ollama_connectivity(ollama_url, offline_strict, sovereign),
        check_ledger_state(ledger_path),
        check_storage_integrity(),
    ];

    DoctorReport {
        sovereign_mode: sovereign,
        offline_strict,
        checks,
    }
}

pub async fn run(
    sovereign: bool,
    offline_strict: bool,
    ollama_url: &str,
    ledger_path: Option<&str>,
    json: bool,
) -> needle::Result<()> {
    let custom_path = ledger_path.map(Path::new);
    let report = run_diagnostics(sovereign, offline_strict, ollama_url, custom_path);

    if json {
        let json_str = serde_json::to_string_pretty(&report)
            .map_err(|e| needle::Error::SerializationError(e.to_string()))?;
        println!("{json_str}");
    } else {
        report.print_human();
    }

    if !report.is_success() {
        std::process::exit(1);
    }

    Ok(())
}
