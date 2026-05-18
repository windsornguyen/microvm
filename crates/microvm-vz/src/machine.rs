// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! VM lifecycle state machine and Virtualization.framework wrapper.

use std::path::PathBuf;

use crate::VzError;

/// VM lifecycle phases. Transitions enforced by [`VmProtocol`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmPhase {
    Stopped,
    Starting,
    Running,
    Paused,
    Stopping,
}

/// Observations that drive VM phase transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmObs {
    StartRequested,
    BootCompleted,
    PauseRequested,
    ResumeRequested,
    StopRequested,
    Stopped,
}

/// DFA for VM lifecycle.
pub struct VmProtocol;

impl VmProtocol {
    pub fn initial() -> VmPhase {
        VmPhase::Stopped
    }

    pub fn transition(phase: VmPhase, obs: VmObs) -> Option<VmPhase> {
        match (phase, obs) {
            (VmPhase::Stopped, VmObs::StartRequested) => Some(VmPhase::Starting),
            (VmPhase::Starting, VmObs::BootCompleted) => Some(VmPhase::Running),
            (VmPhase::Running, VmObs::PauseRequested) => Some(VmPhase::Paused),
            (VmPhase::Paused, VmObs::ResumeRequested) => Some(VmPhase::Running),
            (VmPhase::Running, VmObs::StopRequested) => Some(VmPhase::Stopping),
            (VmPhase::Paused, VmObs::StopRequested) => Some(VmPhase::Stopping),
            (VmPhase::Stopping, VmObs::Stopped) => Some(VmPhase::Stopped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub cpus: u32,
    pub memory_bytes: u64,
    pub kernel: PathBuf,
    pub kernel_cmdline: Vec<String>,
    pub rootfs: PathBuf,
    pub rosetta: bool,
    pub nested_virtualization: bool,
}

impl VmConfig {
    pub fn validate(&self) -> Result<(), VzError> {
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
        if self.cpus == 0 {
            return Err(VzError::InvalidConfig("cpus must be > 0".into()));
        }
        if self.memory_bytes < 64 * 1024 * 1024 {
            return Err(VzError::InvalidConfig("memory must be >= 64 MiB".into()));
        }
        Ok(())
    }
}

/// A running (or stopped) virtual machine instance.
pub struct VmInstance {
    config: VmConfig,
    phase: VmPhase,
}

impl VmInstance {
    pub fn new(config: VmConfig) -> Result<Self, VzError> {
        config.validate()?;
        Ok(Self {
            config,
            phase: VmProtocol::initial(),
        })
    }

    pub fn phase(&self) -> VmPhase {
        self.phase
    }

    pub fn config(&self) -> &VmConfig {
        &self.config
    }

    fn advance(&mut self, obs: VmObs) -> Result<VmPhase, VzError> {
        let next = VmProtocol::transition(self.phase, obs).ok_or(VzError::InvalidState {
            expected: obs.expected_phase(),
            actual: self.phase.name(),
        })?;
        self.phase = next;
        Ok(next)
    }

    pub async fn start(&mut self) -> Result<(), VzError> {
        self.advance(VmObs::StartRequested)?;
        // TODO: VZVirtualMachine create + start via objc2-virtualization
        self.advance(VmObs::BootCompleted)?;
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), VzError> {
        self.advance(VmObs::StopRequested)?;
        // TODO: VZVirtualMachine stop
        self.advance(VmObs::Stopped)?;
        Ok(())
    }

    pub async fn pause(&mut self) -> Result<(), VzError> {
        self.advance(VmObs::PauseRequested)?;
        Ok(())
    }

    pub async fn resume(&mut self) -> Result<(), VzError> {
        self.advance(VmObs::ResumeRequested)?;
        Ok(())
    }

    pub async fn checkpoint(&mut self, path: &std::path::Path) -> Result<(), VzError> {
        self.advance(VmObs::PauseRequested)?;
        // TODO: saveMachineState(to: path)
        let _path = path;
        self.advance(VmObs::ResumeRequested)?;
        Ok(())
    }

    pub async fn restore(&mut self, path: &std::path::Path) -> Result<(), VzError> {
        if !path.exists() {
            return Err(VzError::InvalidConfig(format!(
                "checkpoint not found: {}",
                path.display()
            )));
        }
        self.advance(VmObs::PauseRequested)?;
        // TODO: restoreMachineState(from: path)
        self.advance(VmObs::ResumeRequested)?;
        Ok(())
    }
}

impl VmPhase {
    fn name(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Stopping => "stopping",
        }
    }
}

impl VmObs {
    fn expected_phase(self) -> &'static str {
        match self {
            Self::StartRequested => "stopped",
            Self::BootCompleted => "starting",
            Self::PauseRequested => "running",
            Self::ResumeRequested => "paused",
            Self::StopRequested => "running or paused",
            Self::Stopped => "stopping",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- DFA transition invariants ---

    #[test]
    fn invariant_happy_path_start_stop() {
        let mut phase = VmProtocol::initial();
        for (obs, expected) in [
            (VmObs::StartRequested, VmPhase::Starting),
            (VmObs::BootCompleted, VmPhase::Running),
            (VmObs::StopRequested, VmPhase::Stopping),
            (VmObs::Stopped, VmPhase::Stopped),
        ] {
            phase = VmProtocol::transition(phase, obs)
                .unwrap_or_else(|| panic!("rejected {obs:?} from {phase:?}"));
            assert_eq!(phase, expected);
        }
    }

    #[test]
    fn invariant_happy_path_pause_resume() {
        let mut phase = VmPhase::Running;
        phase = VmProtocol::transition(phase, VmObs::PauseRequested).unwrap();
        assert_eq!(phase, VmPhase::Paused);
        phase = VmProtocol::transition(phase, VmObs::ResumeRequested).unwrap();
        assert_eq!(phase, VmPhase::Running);
    }

    #[test]
    fn invariant_stop_from_paused() {
        let phase = VmProtocol::transition(VmPhase::Paused, VmObs::StopRequested);
        assert_eq!(phase, Some(VmPhase::Stopping));
    }

    #[test]
    fn design_cannot_pause_stopped_vm() {
        assert!(VmProtocol::transition(VmPhase::Stopped, VmObs::PauseRequested).is_none());
    }

    #[test]
    fn design_cannot_resume_running_vm() {
        assert!(VmProtocol::transition(VmPhase::Running, VmObs::ResumeRequested).is_none());
    }

    #[test]
    fn design_cannot_start_running_vm() {
        assert!(VmProtocol::transition(VmPhase::Running, VmObs::StartRequested).is_none());
    }

    #[test]
    fn design_cannot_double_stop() {
        assert!(VmProtocol::transition(VmPhase::Stopped, VmObs::StopRequested).is_none());
    }

    // --- VmConfig validation ---

    #[test]
    fn invariant_zero_cpus_rejected() {
        let config = VmConfig {
            cpus: 0,
            memory_bytes: 128 * 1024 * 1024,
            kernel: PathBuf::from("/dev/null"),
            kernel_cmdline: vec![],
            rootfs: PathBuf::from("/dev/null"),
            rosetta: false,
            nested_virtualization: false,
        };
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }

    #[test]
    fn invariant_tiny_memory_rejected() {
        let config = VmConfig {
            cpus: 1,
            memory_bytes: 1024,
            kernel: PathBuf::from("/dev/null"),
            kernel_cmdline: vec![],
            rootfs: PathBuf::from("/dev/null"),
            rosetta: false,
            nested_virtualization: false,
        };
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }

    #[test]
    fn invariant_missing_kernel_rejected() {
        let config = VmConfig {
            cpus: 1,
            memory_bytes: 128 * 1024 * 1024,
            kernel: PathBuf::from("/nonexistent/kernel"),
            kernel_cmdline: vec![],
            rootfs: PathBuf::from("/dev/null"),
            rosetta: false,
            nested_virtualization: false,
        };
        assert!(matches!(config.validate(), Err(VzError::InvalidConfig(_))));
    }
}
