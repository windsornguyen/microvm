# microvm

Lightweight container runtime for macOS. 515 LOC, 1.3 MB binary.

Uses Apple's Virtualization.framework to boot Linux VMs directly -- no Docker Desktop, no background daemon, no bundled VMM.

## Install

```
curl -fsSL https://raw.githubusercontent.com/windsornguyen/microvm/main/install.sh | sh
```

Or a specific version: `curl -fsSL ... | sh -s v0.1.0`

**From source:**

```
cargo build --release
codesign --sign - --entitlements entitlements.plist --force target/release/microvm
cp target/release/microvm ~/.local/bin/microvm
```

The codesign step is required. Virtualization.framework needs the `com.apple.security.virtualization` entitlement.

## Usage

```
microvm boot --kernel <path> --rootfs <path> [--cpus N] [--memory MiB] [--virtualization] [--checkpoint <path>]
```

`--virtualization` exposes `/dev/kvm` to the guest (nested virtualization).

`--checkpoint` pauses a running VM after boot, writes a machine-state file, then resumes it.

## Status

`microvm` is alpha: a thin Rust CLI over Apple's Virtualization.framework for Linux microVMs on Apple Silicon.

Today it:

- Boots an ARM64 Linux kernel with an ext4 rootfs.
- Wires serial console, entropy, NAT, vsock, virtio block, and optional nested virtualization.
- Can checkpoint a running VM after boot.
- Has low-level restore bindings, but no supported restore command yet.

The constraint is deliberate: use the primary Apple API directly. If the framework cannot do the thing, fail with a typed error. No shadow VMM, no silent mode switch.

## Roadmap

The central hypothesis is that cold boot should become the rare path. Initialize once, checkpoint the useful machine state, then restore it cheaply and correctly.

- Snapshot/restore: persist the machine identifier with each checkpoint, expose `microvm restore`, enforce stopped -> restoring -> paused -> running, and publish cold-boot vs restore numbers.
- Fast substrate: keep kernel/rootfs minimal, document required config, capture boot logs, and track time to first vsock connection.
- Guest agent: replace shell boot with a tiny vsock control plane for exec, signals, stdio, exit status, and lifecycle events.
- Storage: add virtiofs shares, copy-on-write rootfs creation, and explicit disk cache/sync modes.
- Network: graduate from NAT-only to vmnet-backed addresses, with explicit DNS and port semantics.
- OCI: pull an image, materialize a rootfs, and run its process, no Docker Desktop or shared Linux daemon.
- Accounting: report CPU, memory, disk, and network usage, including the framework's memory-balloon limits.

Non-goals for now: macOS guests, GUI devices, Docker API compatibility, and non-Apple VMM backends.

## Getting a kernel

Any ARM64 Linux kernel in `Image` format works. Three options:

**Ubuntu generic (has KVM built-in):**

```
container run --rm -v /tmp/kernel:/out ubuntu:24.04 bash -c '
  apt-get update -qq && apt-get download linux-image-unsigned-6.8.0-31-generic
  dpkg-deb -x linux-image-*.deb /tmp/ex && cp /tmp/ex/boot/vmlinuz-* /out/vmlinuz'
python3 -c "
import gzip
data = gzip.decompress(open('/tmp/kernel/vmlinuz','rb').read())
open('/tmp/kernel/vmlinux','wb').write(data)"
```

**Kata Containers (no KVM, but lightweight):**

Already installed by `container system start` at:
```
~/Library/Application Support/com.apple.container/kernels/vmlinux-*
```

**Build your own:** clone any arm64 kernel tree, apply your config with `CONFIG_KVM=y`, and `make ARCH=arm64 Image`.

## Getting a rootfs

Any ext4 disk image works.

**From Apple's container (Ubuntu 24.04):**

After `container run ubuntu:24.04`, the snapshot lives at:
```
~/Library/Application Support/com.apple.container/snapshots/<digest>/snapshot
```

**From scratch:**

```
dd if=/dev/zero of=rootfs.ext4 bs=1M count=2048
# format inside a container (macOS has no mkfs.ext4):
container run --rm -v /tmp:/mnt ubuntu:24.04 bash -c 'mkfs.ext4 /mnt/rootfs.ext4'
# mount and populate inside a container, or use debootstrap
```

## /dev/kvm on macOS

Boot with `--virtualization` and a KVM-capable kernel (Ubuntu generic arm64 works):

```
microvm boot \
  --kernel /tmp/kernel/vmlinux \
  --rootfs ~/Library/Application\ Support/com.apple.container/snapshots/<digest>/snapshot \
  --cpus 2 --memory 1024 \
  --virtualization
```

The guest will have `/dev/kvm` available. Requires Apple Silicon and macOS 26+.

## Requirements

- Apple Silicon (M1+)
- macOS 26 (Tahoe)
- Rust 2024 edition

## License

Apache-2.0
