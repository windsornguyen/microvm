use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("container {id} is in wrong state: expected {expected}, got {actual}")]
    InvalidState {
        id: String,
        expected: &'static str,
        actual: String,
    },

    #[error("container not found: {0}")]
    NotFound(String),

    #[error("volume not found: {0}")]
    VolumeNotFound(String),

    #[error("volume in use: {0}")]
    VolumeInUse(String),

    #[error("vm: {0}")]
    Vm(#[from] microvm_vz::VzError),

    #[error("oci: {0}")]
    Oci(#[from] microvm_oci::OciError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
