// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Virtualization.framework exit callbacks bridged to a bounded Tokio watch channel.

use objc2::rc::Retained;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSError, NSObject, NSObjectProtocol};
use objc2_virtualization::{VZVirtualMachine, VZVirtualMachineDelegate};
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub(super) enum MachineExit {
    GuestStopped,
    Failed(String),
}

#[derive(Debug)]
pub(super) struct MachineDelegateIvars {
    exit: watch::Sender<Option<MachineExit>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this class does
    // not implement Drop. VZ invokes both methods on the VM's serial queue.
    #[unsafe(super = NSObject)]
    #[name = "MicrovmVirtualMachineDelegate"]
    #[ivars = MachineDelegateIvars]
    pub(super) struct MachineDelegate;

    // SAFETY: NSObjectProtocol has no additional safety requirements.
    unsafe impl NSObjectProtocol for MachineDelegate {}

    // SAFETY: Both signatures match VZVirtualMachineDelegate exactly.
    unsafe impl VZVirtualMachineDelegate for MachineDelegate {
        #[unsafe(method(guestDidStopVirtualMachine:))]
        unsafe fn guest_did_stop(&self, _vm: &VZVirtualMachine) {
            self.ivars().exit.send_replace(Some(MachineExit::GuestStopped));
        }

        #[unsafe(method(virtualMachine:didStopWithError:))]
        unsafe fn did_stop_with_error(&self, _vm: &VZVirtualMachine, error: &NSError) {
            let message = error.localizedDescription().to_string();
            self.ivars().exit.send_replace(Some(MachineExit::Failed(message)));
        }
    }
);

impl MachineDelegate {
    pub(super) fn new(exit: watch::Sender<Option<MachineExit>>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(MachineDelegateIvars { exit });
        // SAFETY: The signature of NSObject's init method is correct.
        unsafe { msg_send![super(this), init] }
    }
}
