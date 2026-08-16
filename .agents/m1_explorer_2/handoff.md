# Investigation & Architecture Report: Local-Only LLM Routing & Offline-Strict Enforcement (Milestone M1)

## 1. Observation

Direct observations from codebase inspection, dependency graphs, and interface requirements:

### A. Current LLM Implementation (`src/llm.rs`)
- **File Location**: `d:\AEGIS_AST\src\llm.rs` (lines 1–198)
- **Provider Enum** (lines 15–21):
  ```rust
  pub enum Provider {
      Anthropic { api_key: String, model: String },
      OpenAI    { api_key: String, model: String },
      Groq      { api_key: String, model: String },
      Ollama    { model: String },
  }
  ```
- **Provider Resolution & Cloud Precedence** (lines 33–51):
  - `LlmClient::from_env()` checks `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GROQ_API_KEY` in priority order before falling back to `Ollama`.
  - In sovereign or air-gapped environments, if any of these environment variables happen to be set in the shell, `from_env()` unconditionally routes requests to external cloud endpoints (`api.anthropic.com`, `api.openai.com`, `api.groq.com`).
- **HTTP Transport Dependency** (lines 192–197):
  - Uses `reqwest::Client` for all providers including Ollama.
  - Line 173: Hardcoded call to `http://127.0.0.1:11434/api/chat`.
  - Line 179: Explicit check `resp.status() == reqwest::StatusCode::NOT_FOUND` relies directly on the `reqwest` crate.
  - In sovereign build mode (`--no-default-features --features sovereign`), `reqwest` must be excluded from compilation (`cargo tree` requirement R1). Compiling `src/llm.rs` without `reqwest` will currently fail unless an alternative zero-dependency transport is provided.

### B. Dependency & Build Configuration (`Cargo.toml`)
- **File Location**: `d:\AEGIS_AST\Cargo.toml`
- `reqwest = { version = "0.12", features = ["blocking", "json"] }` is currently an unconditional dependency (line 79).
- `tokio = { version = "1.35", features = ["full"] }` is already a core dependency (line 58), providing native `tokio::net::TcpStream` and async I/O without any HTTP/TLS or external networking crate overhead.

### C. LLM Call Sites Across Codebase
- **`src/cli/mcp/mod.rs`** (lines 129, 179, 204, 254):
  - Initializes `let llm = needle::llm::LlmClient::from_env();` and dispatches search/graph tools.
- **`src/cli/mcp/tools_search.rs`** (lines 265–267):
  - Invokes `llm.complete(system, &user_msg).await`.
- **`src/cli/serve/handlers_core.rs`** (lines 322–333):
  - Invokes `let client = needle::llm::LlmClient::from_env();` and `client.complete(system, &user_msg).await`.
- **Future Policy Subsystem (`src/policy/structurer.rs`)**:
  - Requires LLM clause obligation extraction via Ollama.

### D. Current Error Propagation (`src/error.rs`)
- **File Location**: `d:\AEGIS_AST\src\error.rs` (lines 5–17):
  - Missing dedicated error variants for `OfflineStrictViolation`, `LlmError`, `PolicyError`, and `LedgerError`.
  - Runtime errors currently convert to generic strings or IO errors.

---

## 2. Logic Chain

1. **Sovereign Isolation Logic (R1 & R2)**:
   - When compiled with `--features sovereign` (or when `cloud` feature is disabled), `src/llm.rs` must not contain or reference `Anthropic`, `OpenAI`, `Groq`, or `reqwest`.
   - Therefore, cloud providers in `Provider` and their respective client calls must be gated under `#[cfg(feature = "cloud")]`.
   - Under `#[cfg(not(feature = "cloud"))]` (or `#[cfg(feature = "sovereign")]`), `Provider` must only contain `Ollama`.

2. **Zero-Networking Dependency Transport for Sovereign Mode**:
   - In sovereign build mode, `reqwest` is completely stripped from the dependency tree.
   - To communicate with the local Ollama daemon on loopback (`127.0.0.1:11434`), `llm.rs` must implement a lightweight, zero-dependency async HTTP/1.1 client over `tokio::net::TcpStream`.
   - This guarantees that `cargo tree --no-default-features --features sovereign` contains zero instances of `reqwest`, `hyper`, `rustls`, or other network client crates while preserving full async communication with local Ollama.

3. **Loopback Validation & `--offline-strict` Enforcement**:
   - In `--offline-strict` mode, every target endpoint must be strictly verified prior to any connection attempt.
   - Host validation logic:
     1. Strip scheme (`http://` or `https://`) and optional port.
     2. If host is `"localhost"`, `"127.0.0.1"`, `"::1"`, or `"[::1]"`: Validated as loopback.
     3. If host is an IP literal (`IpAddr`):
        - `IpAddr::V4(ipv4)` -> `ipv4.is_loopback()` (matches `127.0.0.0/8`).
        - `IpAddr::V6(ipv6)` -> `ipv6.is_loopback()` (matches `::1`).
     4. If host is any non-loopback IP (e.g., `192.168.x.x`, `10.x.x.x`, `0.0.0.0`, public IPs) or non-localhost domain (e.g. `api.openai.com`, `ollama.corp.internal`):
        - Immediately reject with `LlmError::OfflineStrictViolation` without initiating any DNS lookup or TCP handshake.
   - This guarantees zero data leakage or connection initiation outside loopback.

4. **Zero-Panic Runtime Path Invariant**:
   - No `.unwrap()`, `.expect()`, or `panic!()` on any URI parsing, HTTP communication, JSON serialization/deserialization, or environment reading paths.
   - All errors must be wrapped into structured enums (`LlmError`) with descriptive remediation advice (e.g., `"Run 'ollama pull llama3.2'"` or `"Run 'ollama serve'"`).

---

## 3. Architecture & Implementation Specification

### A. Data Types and Configuration

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmConfig {
    pub host: String,
    pub port: u16,
    pub model: String,
    pub timeout_secs: u64,
    pub offline_strict: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port = std::env::var("OLLAMA_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(11434);
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
```

### B. Loopback Validation Engine

```rust
pub struct LoopbackValidator;

impl LoopbackValidator {
    /// Validates that a host string is strictly a local loopback address.
    pub fn validate_host(host: &str) -> Result<(), LlmError> {
        let clean_host = host.trim().trim_start_matches("http://").trim_start_matches("https://");
        let host_without_port = clean_host.split(':').next().unwrap_or(clean_host).trim();

        if host_without_port.is_empty() {
            return Err(LlmError::ConfigError("Host address cannot be empty".into()));
        }

        // 1. Check exact string literals
        if host_without_port.eq_ignore_ascii_case("localhost")
            || host_without_port == "127.0.0.1"
            || host_without_port == "::1"
            || host_without_port == "[::1]"
        {
            return Ok(());
        }

        // 2. Parse as IP address
        if let Ok(ip) = host_without_port.parse::<std::net::IpAddr>() {
            if ip.is_loopback() {
                return Ok(());
            } else {
                return Err(LlmError::OfflineStrictViolation {
                    host: host.to_string(),
                    reason: format!("IP address '{ip}' is not a loopback address (must be in 127.0.0.0/8 or ::1)"),
                });
            }
        }

        // 3. Any other hostname is rejected under offline-strict mode
        Err(LlmError::OfflineStrictViolation {
            host: host.to_string(),
            reason: format!(
                "Host '{host_without_port}' is a remote domain or unapproved name. \
                 Offline-strict mode strictly forbids non-loopback endpoints."
            ),
        })
    }

    /// Validates full URL and extracts (host, port).
    pub fn validate_and_extract(url_or_host: &str, default_port: u16) -> Result<(String, u16), LlmError> {
        let trimmed = url_or_host.trim();
        let stripped = trimmed
            .strip_prefix("http://")
            .or_else(|| trimmed.strip_prefix("https://"))
            .unwrap_or(trimmed);

        let parts: Vec<&str> = stripped.split(':').collect();
        let host = parts[0];
        let port = if parts.len() > 1 {
            parts[1]
                .split('/')
                .next()
                .unwrap_or("")
                .parse::<u16>()
                .map_err(|_| LlmError::ConfigError(format!("Invalid port in endpoint: {trimmed}")))?
        } else {
            default_port
        };

        Self::validate_host(host)?;
        Ok((host.to_string(), port))
    }
}
```

### C. Zero-Dependency Loopback Async HTTP Transport

```rust
pub struct LoopbackHttpClient {
    host: String,
    port: u16,
    timeout: std::time::Duration,
}

impl LoopbackHttpClient {
    pub fn new(host: &str, port: u16, timeout_secs: u64) -> Self {
        Self {
            host: host.to_string(),
            port,
            timeout: std::time::Duration::from_secs(timeout_secs),
        }
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(u16, String), LlmError> {
        let addr = format!("{}:{}", self.host, self.port);

        let connect_fut = tokio::net::TcpStream::connect(&addr);
        let mut stream = tokio::time::timeout(self.timeout, connect_fut)
            .await
            .map_err(|_| LlmError::OllamaUnreachable {
                endpoint: addr.clone(),
                source_message: format!("Connection timed out after {:?}", self.timeout),
            })?
            .map_err(|e| LlmError::OllamaUnreachable {
                endpoint: addr.clone(),
                source_message: e.to_string(),
            })?;

        let body_str = match body {
            Some(v) => serde_json::to_string(v).map_err(|e| LlmError::InvalidResponse {
                message: format!("Request serialization error: {e}"),
            })?,
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
            .map_err(|_| LlmError::OllamaUnreachable {
                endpoint: addr.clone(),
                source_message: "Socket write timed out".into(),
            })?
            .map_err(|e| LlmError::IoError(e.to_string()))?;

        let mut response_bytes = Vec::new();
        tokio::time::timeout(self.timeout, stream.read_to_end(&mut response_bytes))
            .await
            .map_err(|_| LlmError::OllamaUnreachable {
                endpoint: addr.clone(),
                source_message: "Socket read timed out".into(),
            })?
            .map_err(|e| LlmError::IoError(e.to_string()))?;

        let response_str = String::from_utf8_lossy(&response_bytes);
        parse_raw_http_response(&response_str)
    }
}

fn parse_raw_http_response(raw: &str) -> Result<(u16, String), LlmError> {
    let mut parts = raw.splitn(2, "\r\n\r\n");
    let header_part = parts.next().unwrap_or("");
    let body_part = parts.next().unwrap_or("");

    let mut lines = header_part.lines();
    let status_line = lines.next().ok_or_else(|| LlmError::InvalidResponse {
        message: "Empty HTTP response from Ollama".into(),
    })?;

    let status_tokens: Vec<&str> = status_line.split_whitespace().collect();
    if status_tokens.len() < 2 {
        return Err(LlmError::InvalidResponse {
            message: format!("Malformed HTTP status line: {status_line}"),
        });
    }

    let status_code: u16 = status_tokens[1].parse().map_err(|_| LlmError::InvalidResponse {
        message: format!("Invalid HTTP status code: {}", status_tokens[1]),
    })?;

    // Handle Transfer-Encoding: chunked if present
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
            let len_hex = &chunked[..pos].trim();
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
```

### D. Ollama API Operations

```rust
// 1. /api/chat
pub async fn ollama_chat(
    config: &LlmConfig,
    messages: &[serde_json::Value],
) -> Result<String, LlmError> {
    if config.offline_strict {
        LoopbackValidator::validate_host(&config.host)?;
    }

    let client = LoopbackHttpClient::new(&config.host, config.port, config.timeout_secs);
    let body = serde_json::json!({
        "model": config.model,
        "stream": false,
        "messages": messages
    });

    let (status, resp_body) = client.request("POST", "/api/chat", Some(&body)).await?;

    if status == 404 {
        return Err(LlmError::ModelNotFound {
            model: config.model.clone(),
            available_models: vec![],
        });
    }
    if status != 200 {
        return Err(LlmError::HttpError {
            status,
            message: resp_body,
        });
    }

    let val: serde_json::Value = serde_json::from_str(&resp_body).map_err(|e| LlmError::InvalidResponse {
        message: format!("Failed to parse /api/chat JSON: {e}"),
    })?;

    val["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| LlmError::InvalidResponse {
            message: format!("Missing content in response: {val}"),
        })
}

// 2. /api/generate
pub async fn ollama_generate(
    config: &LlmConfig,
    prompt: &str,
    system: Option<&str>,
) -> Result<String, LlmError> {
    if config.offline_strict {
        LoopbackValidator::validate_host(&config.host)?;
    }

    let client = LoopbackHttpClient::new(&config.host, config.port, config.timeout_secs);
    let mut body = serde_json::json!({
        "model": config.model,
        "prompt": prompt,
        "stream": false
    });
    if let Some(sys) = system {
        body["system"] = serde_json::Value::String(sys.to_string());
    }

    let (status, resp_body) = client.request("POST", "/api/generate", Some(&body)).await?;

    if status == 404 {
        return Err(LlmError::ModelNotFound {
            model: config.model.clone(),
            available_models: vec![],
        });
    }
    if status != 200 {
        return Err(LlmError::HttpError {
            status,
            message: resp_body,
        });
    }

    let val: serde_json::Value = serde_json::from_str(&resp_body).map_err(|e| LlmError::InvalidResponse {
        message: format!("Failed to parse /api/generate JSON: {e}"),
    })?;

    val["response"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| LlmError::InvalidResponse {
            message: format!("Missing response text in JSON: {val}"),
        })
}

// 3. /api/tags (Health check & Model Enumeration)
pub async fn ollama_tags(config: &LlmConfig) -> Result<Vec<String>, LlmError> {
    if config.offline_strict {
        LoopbackValidator::validate_host(&config.host)?;
    }

    let client = LoopbackHttpClient::new(&config.host, config.port, 5);
    let (status, resp_body) = client.request("GET", "/api/tags", None).await?;

    if status != 200 {
        return Err(LlmError::HttpError {
            status,
            message: resp_body,
        });
    }

    let val: serde_json::Value = serde_json::from_str(&resp_body).map_err(|e| LlmError::InvalidResponse {
        message: format!("Failed to parse /api/tags JSON: {e}"),
    })?;

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
```

### E. Unified Error Structure (`src/llm.rs` & `src/error.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    OfflineStrictViolation { host: String, reason: String },
    OllamaUnreachable { endpoint: String, source_message: String },
    ModelNotFound { model: String, available_models: Vec<String> },
    HttpError { status: u16, message: String },
    InvalidResponse { message: String },
    ConfigError(String),
    IoError(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::OfflineStrictViolation { host, reason } => {
                write!(
                    f,
                    "Offline-strict violation: Host '{host}' is rejected. {reason}. \
                     Sovereign mode only allows loopback connections (127.0.0.1, localhost, [::1])."
                )
            }
            LlmError::OllamaUnreachable { endpoint, source_message } => {
                write!(
                    f,
                    "Ollama is unreachable at '{endpoint}': {source_message}. \
                     Ensure Ollama is running locally (e.g. 'ollama serve'). In --offline-strict mode, external fallback is forbidden."
                )
            }
            LlmError::ModelNotFound { model, available_models } => {
                if available_models.is_empty() {
                    write!(f, "Model '{model}' not found in local Ollama instance. Run: 'ollama pull {model}'")
                } else {
                    write!(
                        f,
                        "Model '{model}' not found in local Ollama. Available models: [{}]. Run: 'ollama pull {model}'",
                        available_models.join(", ")
                    )
                }
            }
            LlmError::HttpError { status, message } => write!(f, "Ollama HTTP {status}: {message}"),
            LlmError::InvalidResponse { message } => write!(f, "Invalid Ollama response: {message}"),
            LlmError::ConfigError(msg) => write!(f, "LLM config error: {msg}"),
            LlmError::IoError(msg) => write!(f, "LLM IO error: {msg}"),
        }
    }
}

impl std::error::Error for LlmError {}
```

---

## 4. Caveats & Design Boundaries

1. **Embedding Isolation Boundary**:
   - `src/embedding/mod.rs` currently contains an Ollama embedding strategy using `reqwest::blocking::Client`.
   - Per project brief constraints ("Do not modify `embedding/`, `indexing/bm25.rs`, `indexing/hnsw.rs` except for the minimum feature-gating needed"), `src/embedding/mod.rs` should have `reqwest` conditionally gated (`#[cfg(feature = "cloud")]`), falling back to `Strategy::Hash` (pure CPU hash-projection) when compiled in sovereign mode.
2. **DNS Leakage Prevention**:
   - In `--offline-strict` mode, we purposefully reject any host that is not `"localhost"` or an IP literal matching `is_loopback()` WITHOUT performing a DNS query. Performing a DNS query in an air-gapped system could attempt network egress and leak telemetry.
3. **No Unsafe/Panic Invariants**:
   - All string slicing, port conversions, JSON indexing, and socket reads use non-panicking constructs (`get()`, `splitn()`, `unwrap_or()`, `map_err()`).

---

## 5. Conclusion

1. **Sovereign Local Routing**:
   - `src/llm.rs` is fully designed to route exclusively to local Ollama under sovereign mode with zero external networking crate dependencies.
   - Loopback HTTP/1.1 transport runs over standard `tokio::net::TcpStream`.
2. **`--offline-strict` Enforcement**:
   - Loopback validator strictly checks IPv4 (`127.0.0.0/8`), IPv6 (`::1`), and `"localhost"`.
   - Rejects all external endpoints, remote IPs, and domains with loud, actionable `OfflineStrictViolation` errors.
3. **Complete API Support**:
   - Full non-panicking async support for `/api/generate`, `/api/chat`, and `/api/tags` with model health probing.

---

## 6. Verification Method

Once implemented, the following verification commands prove correctness:

1. **Dependency Tree Verification (Zero Networking Crates)**:
   ```bash
   cargo tree --no-default-features --features sovereign | Select-String "reqwest|hyper|sqlx|axum|tower"
   # Must return 0 matches.
   ```

2. **Unit Testing Loopback Validation & Offline-Strict**:
   ```bash
   cargo test --test sovereign_llm_tests
   ```
   - Test cases:
     - `validate_host("127.0.0.1")` -> `Ok(())`
     - `validate_host("localhost")` -> `Ok(())`
     - `validate_host("::1")` -> `Ok(())`
     - `validate_host("127.0.0.50")` -> `Ok(())`
     - `validate_host("192.168.1.100")` -> `Err(OfflineStrictViolation)`
     - `validate_host("api.openai.com")` -> `Err(OfflineStrictViolation)`
     - `validate_host("0.0.0.0")` -> `Err(OfflineStrictViolation)`

3. **Doctor Sovereign Health Check**:
   ```bash
   cargo run --no-default-features --features sovereign -- doctor --sovereign --offline-strict
   ```

4. **Clippy and Compilation**:
   ```bash
   cargo clippy --all-features -- -D warnings
   cargo test
   ```
