# Snapshots

## Creating a snapshot

```
$ microvm boot --kernel vmlinux --rootfs rootfs.ext4 --snapshot ./my-snapshot
```

This boots the VM, pauses it after startup, writes machine state to the snapshot
directory, then resumes the VM.

## Snapshot format

```
my-snapshot/
  config.json      VM config + machine identifier
  metadata.json    schema version, host info, resource checksums
  machine-id       raw VZGenericMachineIdentifier bytes
  machine-state    opaque Virtualization.framework saved state
```

`machine-state` is Apple's opaque VM state. Kernel, rootfs, and disk images
are external resources -- `metadata.json` records their canonical paths, sizes,
and modification times.

## Restoring

```
$ microvm restore --from ./my-snapshot
```

Restore validates that:

- External resources (kernel, rootfs, disks) still exist at their canonical paths
- File sizes and modification times match what was recorded
- Memory and CPU requirements are within current host limits

If any of these checks fail, restore is rejected with a specific error.

## Portability

Paths are canonicalized at snapshot creation time. Restoring from a different
working directory works as long as the kernel and rootfs haven't moved.

Snapshots are not portable across machines. The `machine-state` payload is
tied to the Apple Silicon hardware and macOS version.
