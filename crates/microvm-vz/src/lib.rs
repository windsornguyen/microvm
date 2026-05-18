// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Virtualization.framework bindings for microvm.

mod error;
mod ffi;
mod machine;

pub use error::VzError;
pub use machine::{VmConfig, VmInstance, VmPhase};
