# Wishlist: microvm as local DHV

Goal: make `microvm` a Mac-only stand-in for Dedalus Hypervisor (DHV), so the
unmodified DCS host-agent can drive `create / sleep / wake / publish` locally.
The win is not aesthetic. It cuts the canary-bake loop from roughly 10-15 min to
minutes on a laptop, while keeping the same host-agent control path.

This is the build target. Work bottom-up: VZ device parity, then the DHV control
surface, then snapshot/restore, then a Dedalus harness.

## Progress

- [x] Publish alpha crates for name reservation.
- [x] Expose the full generated `objc2-virtualization` surface through
  `microvm-vz`.
- [x] Use `--nested-virt` for guest KVM exposure.
- [x] Add boot-time block devices:
  `DiskAttachment { path, serial, read_only }`.
- [x] Set VZ block device identifiers from disk serials.
- [x] Add boot-time virtiofs shares:
  `FsShare { tag, host_path, read_only }`.
- [x] Enable a VZ virtio socket device in the boot configuration.
- [x] Wrap VZ pause, resume, save state, and restore state.
- [ ] Bridge VZ vsock streams to host unix sockets.
- [ ] Implement the DHV CLI and HTTP-over-unix-socket API.
- [ ] Map DHV `add-fs` inputs to VZ host-directory shares without lying about
  the socket semantics.
- [ ] Add entitlement-gated integration tests where a guest observes block,
  virtiofs, and vsock devices.
- [ ] Drive unmodified host-agent through `DHV_BINARY=microvm`.
- [ ] Prove snapshot/restore with a pre-sleep sentinel readable after wake.

## The seam

host-agent spawns DHV as:

```sh
<dhv-binary> --api-socket <unix-path> --ready-fd <N> --event-monitor path=<file> [--restore <restore-args>]
```

The child writes one byte to `--ready-fd` after binding the API socket. No
polling. host-agent then talks HTTP/1.1 over the unix socket:
`PUT /api/v1/vm.<verb>` with JSON bodies. Non-2xx responses must return JSON,
because host-agent surfaces `status` plus `body`.

`--restore <args>` means spawned to load a snapshot. No `--restore` means fresh
spawn, then wait for `vm.boot` and device-add calls.

Point host-agent at microvm with `DHV_BINARY=<microvm>`. Everything else should
be unchanged.

## Phase 1: device parity in `microvm-vz`

`VmConfig` should carry the guest's real boot contract:

```rust
VmConfig {
    cpus,
    memory_bytes,
    kernel,
    kernel_cmdline,
    rootfs,
    disks: Vec<DiskAttachment>,
    shares: Vec<FsShare>,
    nested_virt,
}
```

### Block devices

Done for boot-time configuration:

```rust
DiskAttachment {
    path,
    serial,
    read_only,
}
```

The guest finds cachefiles and pending-log volumes by serial under
`/dev/disk/by-id`. `microvm-vz` sets VZ block device identifiers when `serial`
is present and validates Apple-compatible identifiers.

Missing: guest-visible integration proof.

### virtiofs

Done for boot-time configuration:

```rust
FsShare {
    tag,
    host_path,
    read_only,
}
```

The guest mounts the epoch bundle by tag. `microvm-vz` wires
`VZVirtioFileSystemDeviceConfiguration`,
`VZSharedDirectory`, and `VZSingleDirectoryShare`.

Missing: the DHV `FsConfig` uses a `socket` path for external `virtiofsd`, while
VZ wants a host directory. The local harness must provide a precise mapping,
probably by adding a Mac-only `host_path` field or deriving a directory from the
harness context. Do not pretend the socket is a directory.

### vsock

Partly done: the VZ virtio socket device is present in the boot configuration.

Missing: the load-bearing bridge:

- host to guest: `connect(port) -> stream` for 19090, 19091, 19092.
- guest to host: `listen(port) -> unix socket` for 19093.

Publish depends on this. The guest `dfsd` dials vsock 19093 to reach the host
object-store broker. host-agent dials guest-runtime RPC on 19090.

## Phase 2: DHV control surface

This can live in the current binary or a new `microvm-dhv` crate.

### CLI

Accept and implement:

- `--api-socket <path>`
- `--ready-fd <fd>`
- `--event-monitor path=<file>`
- `--restore <args>`
- `--version`

The event monitor can start minimal, but it must append valid JSON lines.

### API

Implement HTTP/1.1 over the unix socket:

| Route | Action |
| --- | --- |
| `vm.boot` | boot the configured VM |
| `vm.info` | report VZ state and memory |
| `vm.add-fs` | accumulate a pending virtiofs share |
| `vm.add-disk` | accumulate a pending block device |
| `vm.add-vsock` | configure vsock bridging |
| `vm.resize` | report unsupported fields precisely |
| `vm.pause` / `vm.resume` | VZ pause / resume |
| `vm.snapshot` | Phase 3 |
| `vm.restore` | Phase 3 |
| `vm.shutdown` / `vm.delete` | stop VM / tear down |
| `vm.seal-record` | no-op is acceptable if explicit |

VZ mostly wires devices at boot configuration time. Implement `add-fs`,
`add-disk`, and `add-vsock` by accumulating them before `vm.boot` or
`vm.restore`. The lifecycle adds devices before boot, so true hotplug is not the
first invariant to chase.

## Phase 3: snapshot / restore

Target behavior:

- `vm.snapshot(destination_url)`: pause first, save VZ machine state to the
  `file://` destination, then return after the file is durable.
- `--restore <args>` / `vm.restore(source_url)`: restore VZ machine state from
  file, then `vm.resume` restarts vCPUs.

Deferrable but important: Phase 1 plus Phase 2 already exercises the publish/CAS
bug class, because sleep-time publish is a vsock RPC before snapshot. Real
restore is needed for wake proof.

## Phase 4: Dedalus harness

Out of scope for this repo. The Dedalus repo should own a local driver that
starts host-agent with `DHV_BINARY=microvm`, runs the object-store broker over a
local in-memory S3, seeds a base epoch, then drives:

```text
create -> write sentinel -> sleep/publish -> wake -> read sentinel
```

The broker must enforce read-only-base scoping locally. That is the invariant
that catches the write-to-base class canary found too late.

## Acceptance

- Phase 1: entitlement-gated tests prove block-by-serial, virtiofs tag mount,
  and vsock round-trip in a guest.
- Phase 2: unmodified host-agent can spawn microvm, boot a guest with
  virtiofs+vsock+block, and complete a create plus publish barrier.
- Phase 3: snapshot plus restore round-trips, and a sentinel written pre-sleep
  is readable post-wake.
- North star: local create/sleep/wake/publish runs in minutes on a Mac and
  reproduces the `errno=-5` write-to-base and `errno=-22` CAS-conflict bug
  classes when reintroduced.

## Notes

- VZ requires the `com.apple.security.virtualization` entitlement and codesign.
- Use `--nested-virt` for nested KVM exposure.
- Keep this Mac/arm64-only until there is a real reason to generalize.
- `objc2-virtualization` is the default binding substrate. If Apple ships a VZ
  API before the crate exposes it, add a narrow local shim inside `microvm-vz`
  and delete it after upstream catches up.
