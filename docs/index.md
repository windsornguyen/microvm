# microvm

Boot Linux VMs on Apple Silicon. Virtualization.framework, nothing else.

```
$ microvm boot --kernel vmlinux --rootfs rootfs.ext4
booting: 2 cpus, 512 MiB
vm started, press ctrl-c to stop
```

## What it does

microvm boots ARM64 Linux kernels on macOS using Apple's Virtualization.framework directly.
No Docker, no daemon, no bundled VMM.

- Serial console, entropy, NAT, vsock, virtio-block
- Snapshot and restore VM state
- Nested virtualization (`/dev/kvm` in the guest)
- Host resource guards (memory and CPUs capped at 75% of host)

## What it does not do

- macOS guests
- GUI devices
- Docker API compatibility
- Non-Apple VMM backends

## Quick start

```
$ cargo install microvm
$ codesign --sign - --entitlements entitlements.plist --force $(which microvm)
$ microvm boot --kernel vmlinux --rootfs rootfs.ext4
```

See [Installation](getting-started/installation.md) for details.
