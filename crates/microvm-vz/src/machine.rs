// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Minimal Virtualization.framework VM lifecycle.

use std::path::PathBuf;

use crate::VzError;
use crate::ffi::VzHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmPhase {
    Stopped,
    Running,
}

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub cpus: u32,
    pub memory_bytes: u64,
    pub kernel: PathBuf,
    pub kernel_cmdline: Vec<String>,
    pub rootfs: PathBuf,
    pub nested_virt: bool,
}

impl VmConfig {
    pub fn validate(&self) -> Result<(), VzError> {
        if self.cpus == 0 {
            return Err(VzError::InvalidConfig("cpus must be > 0".into()));
        }
        if self.memory_bytes < 64 * 1024 * 1024 {
            return Err(VzError::InvalidConfig("memory must be >= 64 MiB".into()));
        }
        if !self.kernel.exists() {
            return Err(VzError::InvalidConfig(format!(
                "kernel not found: {}",
                self.kernel.display()
            )));
        }
        if !self.rootfs.exists() {
            return Err(VzError::InvalidConfig(format!(
                "rootfs not found: {}",
                self.rootfs.display()
            )));
        }
        Ok(())
    }
}

pub struct VmInstance {
    config: VmConfig,
    handle: Option<VzHandle>,
}

impl VmInstance {
    pub fn new(config: VmConfig) -> Result<Self, VzError> {
        config.validate()?;
        Ok(Self {
            config,
            handle: None,
        })
    }

    #[must_use]
    pub fn phase(&self) -> VmPhase {
        if self.handle.is_some() {
            VmPhase::Running
        } else {
            VmPhase::Stopped
        }
    }

    pub async fn start(&mut self) -> Result<(), VzError> {
        if self.handle.is_some() {
            return Err(VzError::InvalidState {
                expected: "stopped",
                actual: "running",
            });
        }

        let handle = VzHandle::new(&self.config)?;
        handle.start().await?;
        self.handle = Some(handle);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), VzError> {
        let handle = self.handle.as_ref().ok_or(VzError::InvalidState {
            expected: "running",
            actual: "stopped",
        })?;

        handle.stop().await?;
        self.handle = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(cpus: u32, memory_bytes: u64, kernel: &str, rootfs: &str) -> VmConfig {
        VmConfig {
            cpus,
            memory_bytes,
            kernel: PathBuf::from(kernel),
            kernel_cmdline: vec![],
            rootfs: PathBuf::from(rootfs),
            nested_virt: false,
        }
    }

    #[test]
    fn invariant_zero_cpus_rejected() {
        let config = config(0, 128 * 1024 * 1024, "/dev/null", "/dev/null");
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }

    #[test]
    fn invariant_tiny_memory_rejected() {
        let config = config(1, 1024, "/dev/null", "/dev/null");
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }

    #[test]
    fn invariant_missing_kernel_rejected() {
        let config = config(1, 128 * 1024 * 1024, "/nonexistent/kernel", "/dev/null");
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }
}
