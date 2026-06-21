# Quick Start

## Get a kernel

Any ARM64 Linux kernel in `Image` format works.

**From Apple's container CLI:**

```
$ container pull ubuntu:24.04
```

Kernel at `~/Library/Application Support/com.apple.container/kernels/vmlinux-*`.

**From an Ubuntu package:**

```
$ container run --rm -v /tmp/kernel:/out ubuntu:24.04 bash -c '
    apt-get update -qq && apt-get download linux-image-unsigned-6.8.0-31-generic
    dpkg-deb -x linux-image-*.deb /tmp/ex && cp /tmp/ex/boot/vmlinuz-* /out/vmlinuz'
$ python3 -c "
import gzip
data = gzip.decompress(open('/tmp/kernel/vmlinuz','rb').read())
open('/tmp/kernel/vmlinux','wb').write(data)"
```

**Build your own:**

Clone any arm64 kernel tree, configure with `CONFIG_KVM=y`, and `make ARCH=arm64 Image`.

## Get a rootfs

Any ext4 disk image.

**From Apple's container CLI:**

After `container run ubuntu:24.04`, the snapshot is at:

```
~/Library/Application Support/com.apple.container/snapshots/<digest>/snapshot
```

**From scratch:**

```
$ dd if=/dev/zero of=rootfs.ext4 bs=1M count=2048
$ container run --rm -v /tmp:/mnt ubuntu:24.04 bash -c 'mkfs.ext4 /mnt/rootfs.ext4'
```

## Boot

```
$ microvm boot --kernel /tmp/kernel/vmlinux --rootfs rootfs.ext4 --cpus 2 --memory 1024
booting: 2 cpus, 1024 MiB
vm started, press ctrl-c to stop
```
