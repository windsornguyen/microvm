# Nested Virtualization

## Exposing /dev/kvm

```
$ microvm boot \
    --kernel vmlinux \
    --rootfs rootfs.ext4 \
    --cpus 4 --memory 2048 \
    --nested-virt
```

The guest will have `/dev/kvm` available. This requires:

- Apple Silicon (M3+ for hardware nested virt support)
- macOS 26+
- A kernel built with `CONFIG_KVM=y` (Ubuntu generic arm64 works)

## Use cases

- Running VMs inside VMs (QEMU/KVM in the guest)
- Testing hypervisor code locally
- Running Firecracker or cloud-hypervisor inside the guest
