// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VzError {
    #[error("vm in wrong state: expected {expected}, got {actual}")]
    InvalidState {
        expected: &'static str,
        actual: &'static str,
    },

    #[error("virtualization framework: {0}")]
    Framework(String),

    #[error("config validation: {0}")]
    InvalidConfig(String),

    #[error("nested virtualization not supported on this hardware")]
    NestedVirtUnsupported,

    #[error("agent connection timed out after {attempts} attempts")]
    AgentTimeout { attempts: u32 },
}
