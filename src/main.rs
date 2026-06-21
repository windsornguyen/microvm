// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Binary entrypoint and main-thread run loop pump.
// CFRunLoop FFI is inherently unsafe; startup expect is fail-fast.
#![allow(unsafe_code)]

mod cli;
mod snapshot;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // The main thread must pump CFRunLoop for Virtualization.framework.
    // Spawn async work on a background thread, use the main thread's
    // RunLoop for GCD dispatch.
    let (done_tx, done_rx) = std::sync::mpsc::channel::<Result<()>>();

    std::thread::spawn(move || {
        #[allow(clippy::expect_used)] // startup: fail fast if tokio can't init
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        let result = rt.block_on(cli.run());
        let _ = done_tx.send(result);
        // Wake the main RunLoop so it checks the channel.
        // SAFETY: `CFRunLoopGetMain` returns a process-global run loop, and
        // `CFRunLoopStop` is the documented way to wake/stop it from another thread.
        unsafe {
            core_foundation::runloop::CFRunLoopStop(core_foundation::runloop::CFRunLoopGetMain());
        }
    });

    loop {
        // SAFETY: Runs the main run loop in the default mode for a bounded
        // interval; no borrowed Rust data crosses the FFI boundary.
        unsafe {
            core_foundation::runloop::CFRunLoopRunInMode(
                core_foundation::runloop::kCFRunLoopDefaultMode,
                0.1,
                u8::from(false),
            );
        }
        if let Ok(result) = done_rx.try_recv() {
            return result;
        }
    }
}
