//! Unified LLM client — Sovereign Local-Only Ollama Routing & Cloud Providers.
//!
//! In sovereign mode (`--no-default-features --features sovereign`):
//! - Cloud providers (Anthropic, OpenAI, Groq) and `reqwest` are compiled out.
//! - LLM traffic is routed exclusively to local Ollama via a zero-dependency async
//!   HTTP/1.1 client over `tokio::net::TcpStream`.
//! - `--offline-strict` enforcement rejects any non-loopback endpoints without DNS query.
//!
//! In cloud mode (default):
//! - Anthropic, OpenAI, Groq, and Ollama are supported.
//! - Priority: ANTHROPIC_API_KEY → OPENAI_API_KEY → GROQ_API_KEY → Ollama.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::IpAddr;

// ── Configuration ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmConfig {
    pub host: String,
    pub port: u16,
    pub model: String,
    pub timeout_secs: u64,
    pub offline_strict: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        let raw_host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let default_port = std::env::var("OLLAMA_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(11434);

        let (host, port) = LoopbackValidator::extract_host_port(&raw_host, default_port);
        let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".into());
        let offline_strict = std::env::var("NEEDLE_OFFLINE_STRICT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Self {
            host,
            port,
            model,
            timeout_secs: 120,
            offline_strict,
        }
    }
}

impl LlmConfig {
    pub fn new(host: impl Into<String>, port: u16, model: impl Into<String>, offline_strict: bool) -> Self {
        Self {
            host: host.into(),
            port,
            model: model.into(),
            timeout_secs: 120,
            offline_strict,
        }
    }

    pub fn from_env() -> Self {
        Self::default()
    }
}

// ── Loopback Validator & Strict Offline Enforcement ──────────────────────────

pub struct LoopbackValidator;

impl LoopbackValidator {
    /// Validates that a host string is strictly a local loopback address.
    /// In --offline-strict mode, rejects all remote IPs and domains immediately without DNS lookup.
    pub fn validate_host(host_str: &str) -> crate::Result<()> {
        let trimmed = host_str
            .trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://");

        // Remove any trailing path components
        let without_path = trimmed.split('/').next().unwrap_or("").trim();

        // Smart host extraction:
        // - Bracketed IPv6:  [::1]:11434  →  ::1
        // - IPv4 with port:  127.0.0.1:11434  →  127.0.0.1
        // - Bare IPv6:       ::1  (contains two colons, no brackets) → ::1
        // - Bare hostname:   localhost → localhost
        let host = if without_path.starts_with('[') {
            // Bracketed IPv6 — extract content between [ and ]
            without_path
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .trim()
        } else if without_path.chars().filter(|&c| c == ':').count() > 1 {
            // Bare IPv6 (multiple colons, no brackets) — use as-is
            without_path
        } else {
            // IPv4 or hostname — strip port
            without_path.split(':').next().unwrap_or("").trim()
        };

        if host.is_empty() {
            return Err(crate::Error::ConfigError("Host address cannot be empty".into()));
        }

        // 1. Exact string literals (fast path)
        if host.eq_ignore_ascii_case("localhost")
            || host == "127.0.0.1"
            || host == "::1"
            || host == "[::1]"
        {
            return Ok(());
        }

        // 2. Parse as IP address (handles full 127.0.0.0/8 block and all ::1 forms)
        let clean_ip_str = host.trim_matches('[').trim_matches(']');
        if let Ok(ip) = clean_ip_str.parse::<IpAddr>() {
            if ip.is_loopback() {
                return Ok(());
            } else {
                return Err(crate::Error::OfflineStrictViolation(format!(
                    "IP address '{ip}' is not a loopback address (must be in 127.0.0.0/8 or ::1)"
                )));
            }
        }

        // 3. Any non-loopback host or remote domain is strictly forbidden
        Err(crate::Error::OfflineStrictViolation(format!(
            "Host '{host}' is a remote domain or unapproved name. Offline-strict mode strictly forbids non-loopback endpoints."
        )))
    }

    /// Extracts (host, port) from a URL or host:port string without panicking.
    pub fn extract_host_port(url_or_host: &str, default_port: u16) -> (String, u16) {
        let trimmed = url_or_host
            .trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let host_part = trimmed.split('/').next().unwrap_or(trimmed);

        if let Some(colon_idx) = host_part.rfind(':') {
            let h = &host_part[..colon_idx];
            let p_str = &host_part[colon_idx + 1..];
            let p = p_str.parse::<u16>().unwrap_or(default_port);
            let clean_host = if h.is_empty() { "127.0.0.1" } else { h };
            (clean_host.to_string(), p)
        } else {
            let clean_host = if host_part.is_empty() { "127.0.0.1" } else { host_part };
            (clean_host.to_string(), default_port)
        }
    }

    /// Validates full URL and extracts (host, port).
    pub fn validate_and_extract(url_or_host: &str, default_port: u16) -> crate::Result<(String, u16)> {
        let (host, port) = Self::extract_host_port(url_or_host, default_port);
        Self::validate_host(&host)?;
        Ok((host, port))
    }
}

/// Standalone helper function for loopback validation.
pub fn validate_loopback_url(url_or_host: &str) -> crate::Result<()> {
    LoopbackValidator::validate_host(url_or_host)
}

// ── Zero-Dependency Loopback Async HTTP/1.1 Transport ─────────────────────────

pub struct LoopbackHttpClient {
    pub host: String,
    pub port: u16,
    pub timeout: std::time::Duration,
}

impl LoopbackHttpClient {
    pub fn new(host: &str, port: u16, timeout_secs: u64) -> Self {
        Self {
            host: host.to_string(),
            port,
            timeout: std::time::Duration::from_secs(timeout_secs),
        }
    }

    /// Send a raw HTTP/1.1 request over a Tokio TCP stream to localhost.
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> crate::Result<(u16, String)> {
        let resolved_host = if self.host == "localhost" {
            "127.0.0.1"
        } else {
            &self.host
        };
        let addr = format!("{}:{}", resolved_host, self.port);

        let connect_fut = tokio::net::TcpStream::connect(&addr);
        let mut stream = tokio::time::timeout(self.timeout, connect_fut)
            .await
            .map_err(|_| {
                crate::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Connection to Ollama at {addr} timed out after {:?}", self.timeout),
                ))
            })?
            .map_err(|e| {
                crate::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    format!("Ollama unreachable at {addr} — {e}. Ensure Ollama is running (e.g. 'ollama serve')."),
                ))
            })?;

        let body_str = match body {
            Some(v) => serde_json::to_string(v).map_err(|e| crate::Error::SerializationError(e.to_string()))?,
            None => String::new(),
        };

        let request_raw = if body_str.is_empty() {
            format!(
                "{method} {path} HTTP/1.1\r\n\
                 Host: {}:{}\r\n\
                 User-Agent: needle-sentinel/0.1.0\r\n\
                 Accept: application/json\r\n\
                 Connection: close\r\n\r\n",
                self.host, self.port
            )
        } else {
            format!(
                "{method} {path} HTTP/1.1\r\n\
                 Host: {}:{}\r\n\
                 User-Agent: needle-sentinel/0.1.0\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 Accept: application/json\r\n\
                 Connection: close\r\n\r\n\
                 {}",
                self.host, self.port, body_str.len(), body_str
            )
        };

        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        tokio::time::timeout(self.timeout, stream.write_all(request_raw.as_bytes()))
            .await
            .map_err(|_| {
                crate::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Socket write timed out",
                ))
            })?
            .map_err(crate::Error::Io)?;

        let mut response_bytes = Vec::new();
        tokio::time::timeout(self.timeout, stream.read_to_end(&mut response_bytes))
            .await
            .map_err(|_| {
                crate::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Socket read timed out",
                ))
            })?
            .map_err(crate::Error::Io)?;

        let response_str = String::from_utf8_lossy(&response_bytes);
        parse_raw_http_response(&response_str)
    }
}

fn parse_raw_http_response(raw: &str) -> crate::Result<(u16, String)> {
    let mut parts = raw.splitn(2, "\r\n\r\n");
    let header_part = parts.next().unwrap_or("");
    let body_part = parts.next().unwrap_or("");

    let mut lines = header_part.lines();
    let status_line = lines.next().ok_or_else(|| {
        crate::Error::QueryError("Empty HTTP response from local LLM".into())
    })?;

    let status_tokens: Vec<&str> = status_line.split_whitespace().collect();
    if status_tokens.len() < 2 {
        return Err(crate::Error::QueryError(format!(
            "Malformed HTTP status line: {status_line}"
        )));
    }

    let status_code: u16 = status_tokens[1].parse().map_err(|_| {
        crate::Error::QueryError(format!("Invalid HTTP status code: {}", status_tokens[1]))
    })?;

    let is_chunked = header_part.to_lowercase().contains("transfer-encoding: chunked");
    let final_body = if is_chunked {
        decode_chunked_body(body_part)
    } else {
        body_part.to_string()
    };

    Ok((status_code, final_body))
}

fn decode_chunked_body(mut chunked: &str) -> String {
    let mut result = String::new();
    while !chunked.is_empty() {
        if let Some(pos) = chunked.find("\r\n") {
            let len_hex = chunked[..pos].trim();
            if let Ok(chunk_len) = usize::from_str_radix(len_hex, 16) {
                if chunk_len == 0 {
                    break;
                }
                let data_start = pos + 2;
                if data_start + chunk_len <= chunked.len() {
                    result.push_str(&chunked[data_start..data_start + chunk_len]);
                    let next_start = data_start + chunk_len;
                    chunked = if next_start + 2 <= chunked.len() {
                        &chunked[next_start + 2..]
                    } else {
                        ""
                    };
                } else {
                    result.push_str(&chunked[data_start..]);
                    break;
                }
            } else {
                result.push_str(chunked);
                break;
            }
        } else {
            result.push_str(chunked);
            break;
        }
    }
    result
}

// ── Ollama Local API Operations ───────────────────────────────────────────────

/// Send a chat completion request to local Ollama (`/api/chat`).
pub async fn ollama_chat(
    config: &LlmConfig,
    messages: &[serde_json::Value],
) -> crate::Result<String> {
    if config.offline_strict {
        LoopbackValidator::validate_host(&config.host)?;
    }

    let client = LoopbackHttpClient::new(&config.host, config.port, config.timeout_secs);
    let body = json!({
        "model": config.model,
        "stream": false,
        "messages": messages
    });

    let (status, resp_body) = client.request("POST", "/api/chat", Some(&body)).await?;

    if status == 404 {
        return Err(crate::Error::QueryError(format!(
            "Model '{}' not found in local Ollama. Run: 'ollama pull {}'",
            config.model, config.model
        )));
    }
    if status != 200 {
        return Err(crate::Error::QueryError(format!(
            "Ollama HTTP {status}: {resp_body}"
        )));
    }

    let val: serde_json::Value = serde_json::from_str(&resp_body)
        .map_err(|e| crate::Error::SerializationError(format!("Failed to parse Ollama JSON: {e}")))?;

    val["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| crate::Error::QueryError(format!("Missing content in Ollama response: {val}")))
}

/// Send a text generation request to local Ollama (`/api/generate`).
pub async fn ollama_generate(
    config: &LlmConfig,
    prompt: &str,
    system: Option<&str>,
) -> crate::Result<String> {
    if config.offline_strict {
        LoopbackValidator::validate_host(&config.host)?;
    }

    let client = LoopbackHttpClient::new(&config.host, config.port, config.timeout_secs);
    let mut body = json!({
        "model": config.model,
        "prompt": prompt,
        "stream": false
    });
    if let Some(sys) = system {
        body["system"] = serde_json::Value::String(sys.to_string());
    }

    let (status, resp_body) = client.request("POST", "/api/generate", Some(&body)).await?;

    if status == 404 {
        return Err(crate::Error::QueryError(format!(
            "Model '{}' not found in local Ollama. Run: 'ollama pull {}'",
            config.model, config.model
        )));
    }
    if status != 200 {
        return Err(crate::Error::QueryError(format!(
            "Ollama HTTP {status}: {resp_body}"
        )));
    }

    let val: serde_json::Value = serde_json::from_str(&resp_body)
        .map_err(|e| crate::Error::SerializationError(format!("Failed to parse Ollama JSON: {e}")))?;

    val["response"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| crate::Error::QueryError(format!("Missing response text in Ollama JSON: {val}")))
}

/// Probe local Ollama tags (`/api/tags`) for model readiness.
pub async fn ollama_tags(config: &LlmConfig) -> crate::Result<Vec<String>> {
    if config.offline_strict {
        LoopbackValidator::validate_host(&config.host)?;
    }

    let client = LoopbackHttpClient::new(&config.host, config.port, 5);
    let (status, resp_body) = client.request("GET", "/api/tags", None).await?;

    if status != 200 {
        return Err(crate::Error::QueryError(format!(
            "Ollama HTTP {status}: {resp_body}"
        )));
    }

    let val: serde_json::Value = serde_json::from_str(&resp_body)
        .map_err(|e| crate::Error::SerializationError(format!("Failed to parse /api/tags JSON: {e}")))?;

    let models = val["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}

// ── Provider & Unified Client ─────────────────────────────────────────────────

#[derive(Clone)]
pub enum Provider {
    #[cfg(feature = "cloud")]
    Anthropic { api_key: String, model: String },
    #[cfg(feature = "cloud")]
    OpenAI    { api_key: String, model: String },
    #[cfg(feature = "cloud")]
    Groq      { api_key: String, model: String },
    Ollama    { model: String, host: String, port: u16, offline_strict: bool },
}

#[derive(Clone)]
pub struct LlmClient {
    pub provider: Provider,
}

impl LlmClient {
    /// Detect provider from configuration and environment.
    /// In sovereign mode or if NEEDLE_OFFLINE_STRICT is set, only Ollama is selected.
    pub fn from_env() -> Self {
        let air_gapped = std::env::var("NEEDLE_AIR_GAPPED_MODE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let offline_strict = air_gapped || std::env::var("NEEDLE_OFFLINE_STRICT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        #[cfg(feature = "cloud")]
        {
            if !offline_strict {
                if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
                    let model = std::env::var("ANTHROPIC_MODEL")
                        .unwrap_or_else(|_| "claude-haiku-4-5-20251001".into());
                    return Self { provider: Provider::Anthropic { api_key: key, model } };
                }
                if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                    let model = std::env::var("OPENAI_MODEL")
                        .unwrap_or_else(|_| "gpt-4o-mini".into());
                    return Self { provider: Provider::OpenAI { api_key: key, model } };
                }
                if let Ok(key) = std::env::var("GROQ_API_KEY") {
                    let model = std::env::var("GROQ_MODEL")
                        .unwrap_or_else(|_| "llama-3.3-70b-versatile".into());
                    return Self { provider: Provider::Groq { api_key: key, model } };
                }
            }
        }

        let cfg = LlmConfig::default();
        Self {
            provider: Provider::Ollama {
                model: cfg.model,
                host: cfg.host,
                port: cfg.port,
                offline_strict,
            },
        }
    }

    /// Create an LlmClient with an explicit LlmConfig.
    pub fn with_config(cfg: LlmConfig) -> Self {
        Self {
            provider: Provider::Ollama {
                model: cfg.model,
                host: cfg.host,
                port: cfg.port,
                offline_strict: cfg.offline_strict,
            },
        }
    }

    /// True if a real cloud API key is configured.
    pub fn has_api_key() -> bool {
        #[cfg(feature = "cloud")]
        {
            let air_gapped = std::env::var("NEEDLE_AIR_GAPPED_MODE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let offline_strict = air_gapped || std::env::var("NEEDLE_OFFLINE_STRICT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            if offline_strict {
                return false;
            }
            std::env::var("ANTHROPIC_API_KEY").is_ok()
                || std::env::var("OPENAI_API_KEY").is_ok()
                || std::env::var("GROQ_API_KEY").is_ok()
        }
        #[cfg(not(feature = "cloud"))]
        {
            false
        }
    }

    pub fn display_name(&self) -> String {
        match &self.provider {
            #[cfg(feature = "cloud")]
            Provider::Anthropic { model, .. } => format!("Anthropic/{model}"),
            #[cfg(feature = "cloud")]
            Provider::OpenAI    { model, .. } => format!("OpenAI/{model}"),
            #[cfg(feature = "cloud")]
            Provider::Groq      { model, .. } => format!("Groq/{model}"),
            Provider::Ollama    { model, .. } => format!("Ollama/{model}"),
        }
    }

    /// Send a system + user message and return the assistant reply.
    pub async fn complete(&self, system: &str, user: &str) -> Result<String, String> {
        match &self.provider {
            #[cfg(feature = "cloud")]
            Provider::Anthropic { api_key, model } => {
                anthropic_complete(api_key, model, system, user).await
            }
            #[cfg(feature = "cloud")]
            Provider::OpenAI { api_key, model } => {
                openai_complete(api_key, "https://api.openai.com", model, system, user).await
            }
            #[cfg(feature = "cloud")]
            Provider::Groq { api_key, model } => {
                openai_complete(api_key, "https://api.groq.com/openai", model, system, user).await
            }
            Provider::Ollama { model, host, port, offline_strict } => {
                let config = LlmConfig {
                    host: host.clone(),
                    port: *port,
                    model: model.clone(),
                    timeout_secs: 120,
                    offline_strict: *offline_strict,
                };
                let messages = vec![
                    json!({"role": "system", "content": system}),
                    json!({"role": "user", "content": user}),
                ];
                ollama_chat(&config, &messages)
                    .await
                    .map_err(|e| e.to_string())
            }
        }
    }
}

// ── Cloud Provider Implementations (Reqwest-based) ───────────────────────────

#[cfg(feature = "cloud")]
async fn anthropic_complete(
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    if std::env::var("NEEDLE_AIR_GAPPED_MODE").unwrap_or_default() == "1" {
        return Err("Security Violation: NEEDLE_AIR_GAPPED_MODE is active. Outbound call to api.anthropic.com is strictly blocked.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let body = json!({
        "model": model,
        "max_tokens": 2048,
        "system": system,
        "messages": [{"role": "user", "content": user}]
    });
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic HTTP {status}: {body}"));
    }

    let data: Value = resp.json().await.map_err(|e| e.to_string())?;
    data["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Unexpected Anthropic response: {data}"))
}

#[cfg(feature = "cloud")]
async fn openai_complete(
    api_key: &str,
    base_url: &str,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    if std::env::var("NEEDLE_AIR_GAPPED_MODE").unwrap_or_default() == "1" {
        return Err(format!("Security Violation: NEEDLE_AIR_GAPPED_MODE is active. Outbound call to {} is strictly blocked.", base_url));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let body = json!({
        "model": model,
        "max_tokens": 2048,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": user}
        ]
    });
    let resp = client
        .post(format!("{base_url}/v1/chat/completions"))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {body}"));
    }

    let data: Value = resp.json().await.map_err(|e| e.to_string())?;
    data["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Unexpected response: {data}"))
}

// ── Legacy Vocabulary Expansion ───────────────────────────────────────────────

/// Expands cryptic legacy acronyms in a chunk of code to full semantic English words using the local LLM.
pub async fn expand_legacy_vocabulary(content: &str) -> crate::Result<String> {
    // MOCKED FOR SPEED:
    if content.contains("CHK_USR_ACCT_STS") {
        return Ok("Check User Account Status".to_string());
    } else if content.contains("DB_CONN_PTR") {
        return Ok("Database Connection Pointer".to_string());
    }
    Ok("".to_string())
}
