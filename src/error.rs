use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidPath(String),
    IndexNotFound(String),
    ChunkingError(String),
    EmbeddingError(String),
    IndexError(String),
    QueryError(String),
    ConfigError(String),
    SerializationError(String),
    PolicyError(String),
    LedgerError(String),
    OfflineStrictViolation(String),
    DoctorError(String),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::InvalidPath(p) => write!(f, "Invalid path: {}", p),
            Error::IndexNotFound(p) => write!(f, "Index not found at {}", p),
            Error::ChunkingError(e) => write!(f, "Chunking error: {}", e),
            Error::EmbeddingError(e) => write!(f, "Embedding error: {}", e),
            Error::IndexError(e) => write!(f, "Index error: {}", e),
            Error::QueryError(e) => write!(f, "Query error: {}", e),
            Error::ConfigError(e) => write!(f, "Config error: {}", e),
            Error::SerializationError(e) => write!(f, "Serialization error: {}", e),
            Error::PolicyError(e) => write!(f, "Policy error: {}", e),
            Error::LedgerError(e) => write!(f, "Ledger error: {}", e),
            Error::OfflineStrictViolation(e) => write!(f, "Offline-strict violation: {}", e),
            Error::DoctorError(e) => write!(f, "Doctor error: {}", e),
            Error::Other(e) => write!(f, "Error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::ConfigError(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::SerializationError(e.to_string())
    }
}

impl From<notify::Error> for Error {
    fn from(e: notify::Error) -> Self {
        Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }
}

impl From<ed25519_dalek::SignatureError> for Error {
    fn from(e: ed25519_dalek::SignatureError) -> Self {
        Error::LedgerError(format!("Ed25519 signature error: {e}"))
    }
}

impl From<hex::FromHexError> for Error {
    fn from(e: hex::FromHexError) -> Self {
        Error::LedgerError(format!("Hex decoding error: {e}"))
    }
}
