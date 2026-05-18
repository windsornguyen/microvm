use thiserror::Error;

#[derive(Debug, Error)]
pub enum OciError {
    #[error("registry: {0}")]
    Registry(String),

    #[error("image not found: {0}")]
    NotFound(String),

    #[error("digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },

    #[error("unsupported media type: {0}")]
    UnsupportedMediaType(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
