// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VzError {
    #[error("vm in wrong state: expected {expected}, got {actual}")]
    InvalidState { expected: &'static str, actual: &'static str },

    #[error("virtualization framework during {operation}: {message}")]
    Framework { operation: &'static str, message: String },

    #[error("virtualization completion dropped during {operation}")]
    CompletionDropped { operation: &'static str },

    #[error("config validation: {0}")]
    InvalidConfig(String),

    #[error("path is not valid UTF-8: {}", path.display())]
    NonUtf8Path { path: PathBuf },
}
