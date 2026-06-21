// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Minimal Virtualization.framework VM lifecycle.
//!
//! All fallible public methods return `VzError`, which is self-documenting
//! via its typed variants.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::path::PathBuf;

use crate::VzError;
use crate::ffi::VzHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmPhase {
    Stopped,
    Paused,
    Running,
}

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub cpus: u32,
    pub memory_bytes: u64,
    pub kernel: PathBuf,
    pub kernel_cmdline: Vec<String>,
    pub rootfs: PathBuf,
    pub disks: Vec<DiskAttachment>,
    pub shares: Vec<FsShare>,
    pub nested_virt: bool,
    pub machine_identifier: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskAttachment {
    pub path: PathBuf,
    pub serial: Option<String>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsShare {
    pub tag: String,
    pub host_path: PathBuf,
    pub read_only: bool,
}

impl VmConfig {
    /// Returns `Err` if the configuration violates hard invariants.
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
        for disk in &self.disks {
            if !disk.path.exists() {
                return Err(VzError::InvalidConfig(format!(
                    "disk not found: {}",
                    disk.path.display()
                )));
            }
            if let Some(serial) = &disk.serial
                && (!serial.is_ascii() || serial.len() > 20)
            {
                return Err(VzError::InvalidConfig(
                    "disk serial must be ASCII and <= 20 bytes".into(),
                ));
            }
        }
        for share in &self.shares {
            if share.tag.is_empty() || share.tag.len() >= 36 {
                return Err(VzError::InvalidConfig(
                    "virtiofs tag must be non-empty and < 36 bytes".into(),
                ));
            }
            if !share.host_path.is_dir() {
                return Err(VzError::InvalidConfig(format!(
                    "share directory not found: {}",
                    share.host_path.display()
                )));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn machine_identifier(&self) -> &[u8] {
        // VmInstance::new unconditionally populates this before returning.
        #[allow(clippy::expect_used)]
        self.machine_identifier
            .as_ref()
            .expect("VmConfig always has a machine identifier after VmInstance::new")
    }
}

pub struct VmInstance {
    config: VmConfig,
    handle: Option<VzHandle>,
    phase: VmPhase,
}

impl VmInstance {
    pub fn new(mut config: VmConfig) -> Result<Self, VzError> {
        if config.machine_identifier.is_none() {
            config.machine_identifier = Some(crate::ffi::new_machine_identifier());
        }
        config.validate()?;
        Ok(Self { config, handle: None, phase: VmPhase::Stopped })
    }

    #[must_use]
    pub fn phase(&self) -> VmPhase {
        self.phase
    }

    #[must_use]
    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    pub async fn start(&mut self) -> Result<(), VzError> {
        self.start_with_save_restore(false).await
    }

    pub async fn start_save_restore(&mut self) -> Result<(), VzError> {
        self.start_with_save_restore(true).await
    }

    async fn start_with_save_restore(&mut self, save_restore: bool) -> Result<(), VzError> {
        if self.handle.is_some() {
            return Err(VzError::InvalidState { expected: "stopped", actual: self.phase.as_str() });
        }

        let handle = if save_restore {
            VzHandle::new_save_restore(&self.config)?
        } else {
            VzHandle::new(&self.config)?
        };
        handle.start().await?;
        self.handle = Some(handle);
        self.phase = VmPhase::Running;
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), VzError> {
        let handle = self
            .handle
            .as_ref()
            .ok_or(VzError::InvalidState { expected: "running or paused", actual: "stopped" })?;

        handle.stop().await?;
        self.handle = None;
        self.phase = VmPhase::Stopped;
        Ok(())
    }

    pub async fn pause(&mut self) -> Result<(), VzError> {
        if self.phase == VmPhase::Paused {
            return Ok(());
        }
        if self.phase != VmPhase::Running {
            return Err(VzError::InvalidState { expected: "running", actual: self.phase.as_str() });
        }
        let handle = self.require_handle()?;
        handle.pause().await?;
        self.phase = VmPhase::Paused;
        Ok(())
    }

    pub async fn resume(&mut self) -> Result<(), VzError> {
        if self.phase == VmPhase::Running {
            return Ok(());
        }
        if self.phase != VmPhase::Paused {
            return Err(VzError::InvalidState { expected: "paused", actual: self.phase.as_str() });
        }
        let handle = self.require_handle()?;
        handle.resume().await?;
        self.phase = VmPhase::Running;
        Ok(())
    }

    pub async fn save_state(&mut self, path: &std::path::Path) -> Result<(), VzError> {
        if self.phase != VmPhase::Paused {
            return Err(VzError::InvalidState { expected: "paused", actual: self.phase.as_str() });
        }
        self.require_handle()?.save_state(path).await
    }

    /// Pause -> save state -> resume. VM keeps running after.
    pub async fn checkpoint(&mut self, path: &std::path::Path) -> Result<(), VzError> {
        if self.phase != VmPhase::Running {
            return Err(VzError::InvalidState { expected: "running", actual: self.phase.as_str() });
        }
        self.pause().await?;
        self.save_state(path).await?;
        self.resume().await?;
        Ok(())
    }

    /// Restore into a stopped VM. Virtualization.framework leaves it paused.
    pub async fn restore(&mut self, path: &std::path::Path) -> Result<(), VzError> {
        if self.handle.is_some() {
            return Err(VzError::InvalidState { expected: "stopped", actual: self.phase.as_str() });
        }
        let handle = VzHandle::new_save_restore(&self.config)?;
        handle.restore_state(path).await?;
        self.handle = Some(handle);
        self.phase = VmPhase::Paused;
        Ok(())
    }

    fn require_handle(&self) -> Result<&VzHandle, VzError> {
        self.handle.as_ref().ok_or(VzError::InvalidState { expected: "running", actual: "stopped" })
    }
}

impl VmPhase {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Paused => "paused",
            Self::Running => "running",
        }
    }
}

#[cfg(test)]
// Tests assert on success/failure; unwrap is the idiomatic assertion mechanism.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn config(cpus: u32, memory_bytes: u64, kernel: &str, rootfs: &str) -> VmConfig {
        VmConfig {
            cpus,
            memory_bytes,
            kernel: PathBuf::from(kernel),
            kernel_cmdline: vec![],
            rootfs: PathBuf::from(rootfs),
            disks: vec![],
            shares: vec![],
            nested_virt: false,
            machine_identifier: None,
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

    #[test]
    fn invariant_disk_serial_is_vz_compatible() {
        let mut config = config(1, 128 * 1024 * 1024, "/dev/null", "/dev/null");
        config.disks.push(DiskAttachment {
            path: PathBuf::from("/dev/null"),
            serial: Some("this-serial-is-far-too-long".to_owned()),
            read_only: true,
        });
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }

    #[test]
    fn invariant_share_tag_is_vz_compatible() {
        let mut config = config(1, 128 * 1024 * 1024, "/dev/null", "/dev/null");
        config.shares.push(FsShare {
            tag: "".to_owned(),
            host_path: PathBuf::from("/"),
            read_only: true,
        });
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }
}
