use std::path::{Path, PathBuf};

/// Content-addressable blob store (OCI layout).
///
/// Layout:
///   <root>/blobs/sha256/<hex>    — raw blobs
///   <root>/ingest/               — in-progress downloads
///   <root>/index.json            — reference -> descriptor index
pub struct ContentStore {
    root: PathBuf,
}

impl ContentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn blob_path(&self, digest: &str) -> PathBuf {
        let hex = digest.strip_prefix("sha256:").unwrap_or(digest);
        self.root.join("blobs").join("sha256").join(hex)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
