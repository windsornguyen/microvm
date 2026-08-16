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
    #[must_use = "configuration validation errors must be handled"]
    pub fn validate(&self) -> Result<(), VzError> {
        self.validate_compute()?;
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

    fn validate_compute(&self) -> Result<(), VzError> {
        const MIN_MEMORY_BYTES: u64 = 64 * 1024 * 1024;

        let limits = crate::ffi::resource_limits();
        let cpus = usize::try_from(self.cpus)
            .map_err(|_| VzError::InvalidConfig("cpu count does not fit this host".into()))?;
        if !limits.cpus.contains(&cpus) {
            return Err(VzError::InvalidConfig(format!(
                "cpus must be between {} and {}",
                limits.cpus.start(),
                limits.cpus.end()
            )));
        }

        let memory_bytes = self.aligned_memory_bytes()?;
        let minimum_memory = (*limits.memory_bytes.start()).max(MIN_MEMORY_BYTES);
        if !(minimum_memory..=*limits.memory_bytes.end()).contains(&memory_bytes) {
            return Err(VzError::InvalidConfig(format!(
                "memory must be between {} and {} bytes",
                minimum_memory,
                limits.memory_bytes.end()
            )));
        }
        Ok(())
    }

    #[must_use = "the aligned memory size must be applied to the VZ configuration"]
    pub(crate) fn aligned_memory_bytes(&self) -> Result<u64, VzError> {
        const MIB: u64 = 1 << 20;

        self.memory_bytes
            .checked_add(MIB - 1)
            .map(|bytes| bytes & !(MIB - 1))
            .ok_or_else(|| VzError::InvalidConfig("memory size overflows MiB alignment".into()))
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

#[must_use = "a VM instance owns resources that require explicit lifecycle management"]
pub struct VmInstance {
    config: VmConfig,
    handle: Option<VzHandle>,
    phase: VmPhase,
}

impl VmInstance {
    #[must_use = "VM construction errors must be handled"]
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

    /// Start the VM.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe after VZ accepts the start operation.
    pub async fn start(&mut self) -> Result<(), VzError> {
        self.start_with_save_restore(false).await
    }

    /// Start the VM with save and restore support enabled.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe after VZ accepts the start operation.
    pub async fn start_save_restore(&mut self) -> Result<(), VzError> {
        self.start_with_save_restore(true).await
    }

    async fn start_with_save_restore(&mut self, save_restore: bool) -> Result<(), VzError> {
        if self.handle.is_some() {
            return Err(VzError::InvalidState { expected: "stopped", actual: self.phase.as_str() });
        }

        let handle = self.create_handle(save_restore).await?;
        handle.start().await?;
        self.handle = Some(handle);
        self.phase = VmPhase::Running;
        Ok(())
    }

    /// Destructively stop the VM.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe after VZ accepts the stop operation.
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

    /// Ask the guest to shut down without destroying its in-memory state.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe after the request is queued on the VZ serial queue.
    pub async fn request_stop(&mut self) -> Result<(), VzError> {
        if self.phase != VmPhase::Running {
            return Err(VzError::InvalidState { expected: "running", actual: self.phase.as_str() });
        }
        if !self.require_handle()?.request_stop().await? {
            self.handle = None;
            self.phase = VmPhase::Stopped;
        }
        Ok(())
    }

    /// Wait until the guest stops itself or Virtualization.framework reports failure.
    ///
    /// # Cancellation safety
    /// Cancellation-safe; a later call observes the retained exit event.
    pub async fn wait_for_stop(&mut self) -> Result<(), VzError> {
        if self.phase == VmPhase::Stopped {
            return Ok(());
        }
        let result = self.require_handle()?.wait_for_exit().await;
        self.handle = None;
        self.phase = VmPhase::Stopped;
        result
    }

    /// Consume a pending guest-stop notification without waiting.
    #[must_use = "a pending exit event must update the VM lifecycle state"]
    pub fn try_finish_stop(&mut self) -> Option<Result<(), VzError>> {
        let result = self.handle.as_ref()?.exit_result()?;
        self.handle = None;
        self.phase = VmPhase::Stopped;
        Some(result)
    }

    /// Pause the VM.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe after VZ accepts the pause operation.
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

    /// Resume the VM.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe after VZ accepts the resume operation.
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

    /// Save the paused VM state to `path`.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe after VZ accepts the save operation.
    pub async fn save_state(&mut self, path: &std::path::Path) -> Result<(), VzError> {
        if self.phase != VmPhase::Paused {
            return Err(VzError::InvalidState { expected: "paused", actual: self.phase.as_str() });
        }
        self.require_handle()?.save_state(path).await
    }

    /// Pause -> save state -> resume. VM keeps running after.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe; cancellation can leave the VM paused.
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
    ///
    /// # Cancellation safety
    /// Not cancellation-safe after VZ accepts the restore operation.
    pub async fn restore(&mut self, path: &std::path::Path) -> Result<(), VzError> {
        if self.handle.is_some() {
            return Err(VzError::InvalidState { expected: "stopped", actual: self.phase.as_str() });
        }
        let handle = self.create_handle(true).await?;
        handle.restore_state(path).await?;
        self.handle = Some(handle);
        self.phase = VmPhase::Paused;
        Ok(())
    }

    async fn create_handle(&self, save_restore: bool) -> Result<VzHandle, VzError> {
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || {
            if save_restore { VzHandle::new_save_restore(&config) } else { VzHandle::new(&config) }
        })
        .await
        .map_err(|source| VzError::TaskJoin { operation: "configure", source })?
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
        let config = config(1, 32 * 1024 * 1024, "/dev/null", "/dev/null");
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }

    #[test]
    fn invariant_framework_cpu_limit_rejected() {
        let config = config(u32::MAX, 128 * 1024 * 1024, "/dev/null", "/dev/null");
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }

    #[test]
    fn invariant_framework_memory_limit_rejected() {
        let config = config(1, u64::MAX, "/dev/null", "/dev/null");
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
            tag: String::new(),
            host_path: PathBuf::from("/"),
            read_only: true,
        });
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }
}
