use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("no addresses available in subnet {0}")]
    SubnetExhausted(String),

    #[error("network not found: {0}")]
    NotFound(String),

    #[error("vmnet: {0}")]
    Vmnet(String),

    #[error("port {0} requires root")]
    PrivilegedPort(u16),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
