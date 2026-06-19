// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Virtualization.framework bindings for microvm.
//!
//! The crate root and `raw` module re-export the complete generated
//! `objc2-virtualization` surface so `microvm-vz` can be used as a general VZ
//! binding crate while the higher-level VM helpers stay small and opinionated.

mod error;
mod ffi;
mod machine;

pub mod raw {
    pub use objc2_virtualization::*;
}

pub use error::VzError;
pub use machine::{DiskAttachment, FsShare, VmConfig, VmInstance, VmPhase};
pub use raw::*;

#[cfg(test)]
mod tests {
    use super::raw;

    fn assert_binding<T>() {}

    #[test]
    fn exposes_full_vz_binding_surface() {
        assert_binding::<raw::VZVirtualMachine>();
        assert_binding::<raw::VZVirtualMachineConfiguration>();
        assert_binding::<raw::VZGenericMachineIdentifier>();
        assert_binding::<raw::VZVirtioFileSystemDeviceConfiguration>();
        assert_binding::<raw::VZSharedDirectory>();
        assert_binding::<raw::VZSingleDirectoryShare>();
        assert_binding::<raw::VZVirtioSocketDevice>();
        assert_binding::<raw::VZVirtioSocketConnection>();
        assert_binding::<raw::VZVirtioSocketListener>();
        assert_binding::<raw::VZVirtioBlockDeviceConfiguration>();
        assert_binding::<raw::VZMemoryBalloonDevice>();
        assert_binding::<raw::VZUSBController>();
        assert_binding::<raw::VZMacOSInstaller>();
        assert_binding::<raw::VZVirtualMachineView>();

        let _ = raw::VZErrorCode::InvalidVirtualMachineState;
        let _ = raw::VZVirtualMachineState::Stopped;
    }
}
