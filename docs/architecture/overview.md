# Architecture

## Crates

```
microvm/                CLI binary
crates/microvm-vz/      Virtualization.framework bindings
```

`microvm-vz` wraps Apple's Objective-C `VZVirtualMachine` API in async Rust
via `objc2-virtualization`. The CLI crate handles argument parsing, host
resource validation, and snapshot persistence.

## Stack

```
microvm boot --kernel vmlinux --rootfs rootfs.ext4
    |
    v
cli.rs          parse args, validate host resources
    |
    v
microvm-vz      VmConfig -> VZVirtualMachineConfiguration
    |
    v
ffi.rs          objc2 bindings, dispatch queue, completion handlers
    |
    v
Virtualization.framework (Apple)
    |
    v
Hardware (Apple Silicon hypervisor)
```

## Design constraints

**Use the primary API directly.** If Virtualization.framework cannot do the
thing, fail with a typed error. No shadow VMM, no silent mode switch.

**Host safety.** Memory and CPU allocation are capped at 75% of host physical
resources. macOS has no OOM killer; over-allocation causes unrecoverable
system hangs.

**Snapshots are external.** Machine state is opaque (Apple's format). Kernel,
rootfs, and disk images are external host resources validated by path, size,
and mtime at restore time.

## Thread model

The main thread pumps `CFRunLoop` for Virtualization.framework's GCD
requirements. Async work runs on a tokio single-threaded runtime on a
background thread. VM operations are dispatched to a serial
`DispatchQueue` owned by the `VzHandle`.
