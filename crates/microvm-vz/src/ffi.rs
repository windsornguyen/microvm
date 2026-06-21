// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Async wrappers around Virtualization.framework's callback-based API.
//!
//! This module is the sole FFI boundary with Objective-C. Every function
//! here touches raw Objective-C pointers and dispatch queues.
#![allow(unsafe_code, unreachable_pub, clippy::expect_used)]

use std::path::Path;
use std::sync::Mutex;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchQueueAttr, DispatchRetained};
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSData, NSError, NSFileHandle, NSString, NSURL};
use objc2_virtualization::{
    VZDirectoryShare, VZDirectorySharingDeviceConfiguration, VZDiskImageCachingMode,
    VZDiskImageStorageDeviceAttachment, VZDiskImageSynchronizationMode,
    VZFileHandleSerialPortAttachment, VZGenericMachineIdentifier, VZGenericPlatformConfiguration,
    VZLinuxBootLoader, VZNATNetworkDeviceAttachment, VZNetworkDeviceAttachment,
    VZSerialPortAttachment, VZSharedDirectory, VZSingleDirectoryShare, VZStorageDeviceAttachment,
    VZStorageDeviceConfiguration, VZVirtioBlockDeviceConfiguration,
    VZVirtioConsoleDeviceSerialPortConfiguration, VZVirtioEntropyDeviceConfiguration,
    VZVirtioFileSystemDeviceConfiguration, VZVirtioNetworkDeviceConfiguration,
    VZVirtioSocketDeviceConfiguration, VZVirtualMachine, VZVirtualMachineConfiguration,
};

use crate::VzError;
use crate::machine::VmConfig;

pub(crate) struct VzHandle {
    vm: Retained<VZVirtualMachine>,
    queue: DispatchRetained<DispatchQueue>,
}

struct QueueBoundVm(Retained<VZVirtualMachine>);

// SAFETY: The wrapper is private and only moved into the VM's serial dispatch
// queue by `VzHandle::with_completion`.
unsafe impl Send for QueueBoundVm {}

impl QueueBoundVm {
    fn submit<F>(self, submit: F, block: &block2::DynBlock<dyn Fn(*mut NSError)>)
    where
        F: FnOnce(&VZVirtualMachine, &block2::DynBlock<dyn Fn(*mut NSError)>),
    {
        submit(&self.0, block);
    }
}

// SAFETY: Public methods enqueue all VZVirtualMachine operations onto the VM's
// serial dispatch queue before touching the Objective-C object.
unsafe impl Send for VzHandle {}
// SAFETY: Shared references only submit work to the serial dispatch queue.
unsafe impl Sync for VzHandle {}

impl VzHandle {
    pub fn new(config: &VmConfig) -> Result<Self, VzError> {
        Self::new_with_save_restore(config, false)
    }

    pub fn new_save_restore(config: &VmConfig) -> Result<Self, VzError> {
        Self::new_with_save_restore(config, true)
    }

    fn new_with_save_restore(config: &VmConfig, save_restore: bool) -> Result<Self, VzError> {
        let queue = DispatchQueue::new("com.microvm.vz", DispatchQueueAttr::SERIAL);
        let vz_config = build_vz_config(config, save_restore)?;
        // SAFETY: `queue` is serial, and all later VM operations are submitted
        // through the same queue.
        let vm = unsafe {
            VZVirtualMachine::initWithConfiguration_queue(
                VZVirtualMachine::alloc(),
                &vz_config,
                &queue,
            )
        };
        Ok(Self { vm, queue })
    }

    pub async fn start(&self) -> Result<(), VzError> {
        self.with_completion("start", |vm, block| {
            // SAFETY: `with_completion` executes this call on the VM queue.
            unsafe { vm.startWithCompletionHandler(block) };
        })
        .await
    }

    pub async fn stop(&self) -> Result<(), VzError> {
        self.with_completion("stop", |vm, block| {
            unsafe { vm.stopWithCompletionHandler(block) };
        })
        .await
    }

    pub async fn pause(&self) -> Result<(), VzError> {
        self.with_completion("pause", |vm, block| {
            unsafe { vm.pauseWithCompletionHandler(block) };
        })
        .await
    }

    pub async fn resume(&self) -> Result<(), VzError> {
        self.with_completion("resume", |vm, block| {
            unsafe { vm.resumeWithCompletionHandler(block) };
        })
        .await
    }

    pub async fn save_state(&self, path: &Path) -> Result<(), VzError> {
        let url_path = path_string(path)?;
        self.with_completion("save_state", move |vm, block| {
            let url = nsurl_from_str(&url_path);
            unsafe { vm.saveMachineStateToURL_completionHandler(&url, block) };
        })
        .await
    }

    pub async fn restore_state(&self, path: &Path) -> Result<(), VzError> {
        let url_path = path_string(path)?;
        self.with_completion("restore_state", move |vm, block| {
            let url = nsurl_from_str(&url_path);
            unsafe { vm.restoreMachineStateFromURL_completionHandler(&url, block) };
        })
        .await
    }

    async fn with_completion<F>(&self, operation: &'static str, submit: F) -> Result<(), VzError>
    where
        F: FnOnce(&VZVirtualMachine, &block2::DynBlock<dyn Fn(*mut NSError)>) + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), VzError>>();
        let vm = QueueBoundVm(self.vm.clone());

        self.queue.exec_async(move || {
            let tx = Mutex::new(Some(tx));
            let block = RcBlock::new(move |err: *mut NSError| {
                let result = completion_result(operation, err);
                let mut tx = tx.lock().expect("completion sender mutex poisoned");
                if let Some(tx) = tx.take() {
                    let _ = tx.send(result);
                }
            });
            vm.submit(submit, &block);
        });

        rx.await.map_err(|_| VzError::CompletionDropped { operation })?
    }
}

pub(crate) fn new_machine_identifier() -> Vec<u8> {
    // SAFETY: Allocates a fresh VZGenericMachineIdentifier and immediately
    // copies its opaque byte representation into Rust-owned memory.
    unsafe { VZGenericMachineIdentifier::new().dataRepresentation().as_bytes_unchecked().to_vec() }
}

fn build_vz_config(
    config: &VmConfig,
    save_restore: bool,
) -> Result<Retained<VZVirtualMachineConfiguration>, VzError> {
    let kernel_url = nsurl_from_path(&config.kernel)?;
    // SAFETY: All objects are freshly allocated Objective-C configuration
    // values. The arrays are retained before being copied into `vz_config`.
    unsafe {
        let vz_config = VZVirtualMachineConfiguration::new();
        vz_config.setCPUCount(config.cpus as usize);
        let aligned = (config.memory_bytes + (1 << 20) - 1) & !((1 << 20) - 1);
        vz_config.setMemorySize(aligned);

        let boot_loader =
            VZLinuxBootLoader::initWithKernelURL(VZLinuxBootLoader::alloc(), &kernel_url);
        let cmdline = config.kernel_cmdline.join(" ");
        boot_loader.setCommandLine(&NSString::from_str(&cmdline));
        vz_config.setBootLoader(Some(&boot_loader));

        let platform = VZGenericPlatformConfiguration::new();
        let machine_identifier = machine_identifier(config.machine_identifier())?;
        platform.setMachineIdentifier(&machine_identifier);
        if config.nested_virt {
            if !VZGenericPlatformConfiguration::isNestedVirtualizationSupported() {
                return Err(VzError::InvalidConfig(
                    "nested virtualization not supported on this hardware".into(),
                ));
            }
            platform.setNestedVirtualizationEnabled(true);
        }
        vz_config.setPlatform(&platform);

        let entropy = VZVirtioEntropyDeviceConfiguration::new();
        vz_config
            .setEntropyDevices(&NSArray::from_retained_slice(&[Retained::into_super(entropy)]));

        // Wire serial console to process stdin/stdout.
        let stdin_handle = NSFileHandle::fileHandleWithStandardInput();
        let stdout_handle = NSFileHandle::fileHandleWithStandardOutput();
        let serial_attachment =
            VZFileHandleSerialPortAttachment::initWithFileHandleForReading_fileHandleForWriting(
                VZFileHandleSerialPortAttachment::alloc(),
                Some(&stdin_handle),
                Some(&stdout_handle),
            );
        let serial_attachment: Retained<VZSerialPortAttachment> =
            Retained::into_super(serial_attachment);
        let serial = VZVirtioConsoleDeviceSerialPortConfiguration::new();
        serial.setAttachment(Some(&serial_attachment));
        vz_config.setSerialPorts(&NSArray::from_retained_slice(&[Retained::into_super(serial)]));

        let socket = VZVirtioSocketDeviceConfiguration::new();
        vz_config.setSocketDevices(&NSArray::from_retained_slice(&[Retained::into_super(socket)]));

        let net_dev = VZVirtioNetworkDeviceConfiguration::new();
        let nat = VZNATNetworkDeviceAttachment::new();
        let nat: Retained<VZNetworkDeviceAttachment> = Retained::into_super(nat);
        net_dev.setAttachment(Some(&nat));
        vz_config
            .setNetworkDevices(&NSArray::from_retained_slice(&[Retained::into_super(net_dev)]));

        let mut storage_devices = Vec::with_capacity(1 + config.disks.len());
        storage_devices.push(block_device(&config.rootfs, false, None)?);
        for disk in &config.disks {
            storage_devices.push(block_device(&disk.path, disk.read_only, disk.serial.as_deref())?);
        }
        vz_config.setStorageDevices(&NSArray::from_retained_slice(&storage_devices));

        let mut directory_shares: Vec<Retained<VZDirectorySharingDeviceConfiguration>> =
            Vec::with_capacity(config.shares.len());
        for share in &config.shares {
            let tag = NSString::from_str(&share.tag);
            VZVirtioFileSystemDeviceConfiguration::validateTag_error(&tag)
                .map_err(|e| VzError::InvalidConfig(e.localizedDescription().to_string()))?;
            let host_url = nsurl_from_path(&share.host_path)?;
            let directory = VZSharedDirectory::initWithURL_readOnly(
                VZSharedDirectory::alloc(),
                &host_url,
                share.read_only,
            );
            let single = VZSingleDirectoryShare::initWithDirectory(
                VZSingleDirectoryShare::alloc(),
                &directory,
            );
            let single_base: Retained<VZDirectoryShare> = Retained::into_super(single);
            let device = VZVirtioFileSystemDeviceConfiguration::initWithTag(
                VZVirtioFileSystemDeviceConfiguration::alloc(),
                &tag,
            );
            device.setShare(Some(&single_base));
            directory_shares.push(Retained::into_super(device));
        }
        if !directory_shares.is_empty() {
            vz_config.setDirectorySharingDevices(&NSArray::from_retained_slice(&directory_shares));
        }

        vz_config
            .validateWithError()
            .map_err(|e| VzError::InvalidConfig(e.localizedDescription().to_string()))?;
        if save_restore {
            vz_config
                .validateSaveRestoreSupportWithError()
                .map_err(|e| VzError::InvalidConfig(e.localizedDescription().to_string()))?;
        }

        Ok(vz_config)
    }
}

fn machine_identifier(bytes: &[u8]) -> Result<Retained<VZGenericMachineIdentifier>, VzError> {
    let data = NSData::with_bytes(bytes);
    // SAFETY: Virtualization.framework validates the opaque data representation
    // and returns nil for invalid bytes.
    unsafe {
        VZGenericMachineIdentifier::initWithDataRepresentation(
            VZGenericMachineIdentifier::alloc(),
            &data,
        )
    }
    .ok_or_else(|| VzError::InvalidConfig("invalid generic machine identifier".into()))
}

fn block_device(
    path: &Path,
    read_only: bool,
    serial: Option<&str>,
) -> Result<Retained<VZStorageDeviceConfiguration>, VzError> {
    let url = nsurl_from_path(path)?;
    // SAFETY: The URL is a validated file URL, the attachment and block
    // configuration are fresh Objective-C objects, and the returned retained
    // storage configuration is copied into the VM configuration before use.
    unsafe {
        let disk = VZDiskImageStorageDeviceAttachment::initWithURL_readOnly_cachingMode_synchronizationMode_error(
            VZDiskImageStorageDeviceAttachment::alloc(),
            &url,
            read_only,
            VZDiskImageCachingMode::Automatic,
            VZDiskImageSynchronizationMode::Full,
        ).map_err(|e| VzError::Framework {
            operation: "disk_attach",
            message: e.localizedDescription().to_string(),
        })?;
        let disk_base: Retained<VZStorageDeviceAttachment> = Retained::into_super(disk);
        let block_dev = VZVirtioBlockDeviceConfiguration::initWithAttachment(
            VZVirtioBlockDeviceConfiguration::alloc(),
            &disk_base,
        );
        if let Some(serial) = serial {
            let serial = NSString::from_str(serial);
            VZVirtioBlockDeviceConfiguration::validateBlockDeviceIdentifier_error(&serial)
                .map_err(|e| VzError::InvalidConfig(e.localizedDescription().to_string()))?;
            block_dev.setBlockDeviceIdentifier(&serial);
        }
        Ok(Retained::into_super(block_dev))
    }
}

fn completion_result(operation: &'static str, err: *mut NSError) -> Result<(), VzError> {
    // SAFETY: Virtualization.framework invokes completion handlers with either
    // null or a live NSError pointer for the duration of the callback.
    let Some(err) = (unsafe { err.as_ref() }) else {
        return Ok(());
    };
    Err(VzError::Framework { operation, message: err.localizedDescription().to_string() })
}

fn nsurl_from_path(path: &Path) -> Result<Retained<NSURL>, VzError> {
    path_string(path).map(|path| nsurl_from_str(&path))
}

fn nsurl_from_str(path: &str) -> Retained<NSURL> {
    NSURL::initFileURLWithPath(NSURL::alloc(), &NSString::from_str(path))
}

fn path_string(path: &Path) -> Result<String, VzError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| VzError::NonUtf8Path { path: path.to_path_buf() })
}
