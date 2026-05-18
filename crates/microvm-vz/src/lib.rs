// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Virtualization.framework bindings for microvm.

mod error;
mod machine;

pub use error::VzError;
pub use machine::{VmConfig, VmInstance, VmObs, VmPhase, VmProtocol};
