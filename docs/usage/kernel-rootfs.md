# Kernel & Rootfs

## Kernel requirements

microvm boots any ARM64 Linux kernel in uncompressed `Image` format.
The kernel must be decompressed -- `vmlinuz` (gzip-compressed) must be
unpacked to `vmlinux` first.

The default kernel command line is:

```
console=hvc0 root=/dev/vda rootfstype=ext4 rw init=/bin/sh
```

Additional arguments can be passed with `--cmdline`.

## Rootfs requirements

Any ext4 disk image. The image is attached as `/dev/vda` via virtio-block.

## Sources

### Apple container CLI

The fastest path. Install Apple's `container` CLI (macOS 26+):

```
$ container pull ubuntu:24.04
```

- Kernel: `~/Library/Application Support/com.apple.container/kernels/vmlinux-*`
- Rootfs: `~/Library/Application Support/com.apple.container/snapshots/<digest>/snapshot`

### Ubuntu package

```
$ apt-get download linux-image-unsigned-6.8.0-31-generic
$ dpkg-deb -x linux-image-*.deb /tmp/ex
$ python3 -c "
import gzip
data = gzip.decompress(open('/tmp/ex/boot/vmlinuz-6.8.0-31-generic','rb').read())
open('vmlinux','wb').write(data)"
```

### Custom kernel

```
$ git clone --depth 1 https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git
$ cd linux
$ make ARCH=arm64 defconfig
$ scripts/config --enable KVM
$ make ARCH=arm64 -j$(nproc) Image
```

The kernel is at `arch/arm64/boot/Image`.
