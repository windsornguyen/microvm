// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Async wrappers around Virtualization.framework's callback-based API.

use std::path::Path;
use std::sync::Mutex;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchQueueAttr, DispatchRetained};
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSError, NSFileHandle, NSString, NSURL};
use objc2_virtualization::{
    VZDiskImageCachingMode, VZDiskImageStorageDeviceAttachment, VZDiskImageSynchronizationMode,
    VZFileHandleSerialPortAttachment, VZGenericPlatformConfiguration, VZLinuxBootLoader,
    VZNATNetworkDeviceAttachment, VZNetworkDeviceAttachment, VZSerialPortAttachment,
    VZStorageDeviceAttachment, VZStorageDeviceConfiguration, VZVirtioBlockDeviceConfiguration,
    VZVirtioConsoleDeviceSerialPortConfiguration, VZVirtioEntropyDeviceConfiguration,
    VZVirtioNetworkDeviceConfiguration, VZVirtioSocketDeviceConfiguration, VZVirtualMachine,
    VZVirtualMachineConfiguration,
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
        let queue = DispatchQueue::new("com.microvm.vz", DispatchQueueAttr::SERIAL);
        let vz_config = build_vz_config(config)?;
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
            // SAFETY: `with_completion` executes this call on the VM queue.
            unsafe { vm.stopWithCompletionHandler(block) };
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

        rx.await
            .map_err(|_| VzError::CompletionDropped { operation })?
    }
}

fn build_vz_config(config: &VmConfig) -> Result<Retained<VZVirtualMachineConfiguration>, VzError> {
    let kernel_url = nsurl_from_path(&config.kernel)?;
    let rootfs_url = nsurl_from_path(&config.rootfs)?;

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
        vz_config.setPlatform(&platform);

        let entropy = VZVirtioEntropyDeviceConfiguration::new();
        vz_config.setEntropyDevices(&NSArray::from_retained_slice(&[Retained::into_super(
            entropy,
        )]));

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
        vz_config.setSerialPorts(&NSArray::from_retained_slice(&[Retained::into_super(
            serial,
        )]));

        let socket = VZVirtioSocketDeviceConfiguration::new();
        vz_config.setSocketDevices(&NSArray::from_retained_slice(&[Retained::into_super(
            socket,
        )]));

        let net_dev = VZVirtioNetworkDeviceConfiguration::new();
        let nat = VZNATNetworkDeviceAttachment::new();
        let nat: Retained<VZNetworkDeviceAttachment> = Retained::into_super(nat);
        net_dev.setAttachment(Some(&nat));
        vz_config.setNetworkDevices(&NSArray::from_retained_slice(&[Retained::into_super(
            net_dev,
        )]));

        let disk: Retained<VZDiskImageStorageDeviceAttachment> =
            VZDiskImageStorageDeviceAttachment::initWithURL_readOnly_cachingMode_synchronizationMode_error(
                VZDiskImageStorageDeviceAttachment::alloc(),
                &rootfs_url,
                false,
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
        let block_as_storage: Retained<VZStorageDeviceConfiguration> =
            Retained::into_super(block_dev);
        vz_config.setStorageDevices(&NSArray::from_retained_slice(&[block_as_storage]));

        vz_config
            .validateWithError()
            .map_err(|e| VzError::InvalidConfig(e.localizedDescription().to_string()))?;

        Ok(vz_config)
    }
}

fn completion_result(operation: &'static str, err: *mut NSError) -> Result<(), VzError> {
    // SAFETY: Virtualization.framework invokes completion handlers with either
    // null or a live NSError pointer for the duration of the callback.
    let Some(err) = (unsafe { err.as_ref() }) else {
        return Ok(());
    };
    Err(VzError::Framework {
        operation,
        message: err.localizedDescription().to_string(),
    })
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
        .ok_or_else(|| VzError::NonUtf8Path {
            path: path.to_path_buf(),
        })
}
