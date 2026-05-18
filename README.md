# microvm

Lightweight container runtime for macOS. 515 LOC, 1.3 MB binary.

Uses Apple's Virtualization.framework to boot Linux VMs directly -- no Docker Desktop, no background daemon, no bundled VMM.

## Install

```
cargo build --release
codesign --sign - --entitlements entitlements.plist --force target/release/microvm
cp target/release/microvm ~/.local/bin/microvm
```

Or: `./scripts/install.sh`

The codesign step is required. Virtualization.framework needs the `com.apple.security.virtualization` entitlement.

## Usage

```
microvm boot --kernel <path> --rootfs <path> [--cpus N] [--memory MiB] [--virtualization]
```

`--virtualization` exposes `/dev/kvm` to the guest (nested virtualization).

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
