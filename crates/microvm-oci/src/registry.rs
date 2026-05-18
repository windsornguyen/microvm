/// OCI registry client wrapping oci-client.
pub struct RegistryClient {
    _default_domain: String,
}

impl RegistryClient {
    pub fn new(default_domain: impl Into<String>) -> Self {
        Self {
            _default_domain: default_domain.into(),
        }
    }
}
