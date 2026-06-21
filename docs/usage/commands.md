# Commands

## boot

```
$ microvm boot --kernel <path> --rootfs <path> [options]
```

| Flag | Default | Description |
| --- | --- | --- |
| `--kernel` | required | Path to ARM64 Linux kernel |
| `--rootfs` | required | Path to ext4 disk image |
| `--cpus` | 2 | Number of vCPUs |
| `--memory` | 512 | Memory in MiB |
| `--nested-virt` | false | Expose `/dev/kvm` to guest |
| `--snapshot` | none | Save snapshot to directory after boot |
| `--cmdline` | none | Extra kernel command line arguments |

Memory is capped at 75% of host physical RAM. CPUs are capped at 75% of host cores.

The VM stops on ctrl-c with a 10-second timeout.

## restore

```
$ microvm restore --from <dir> [--paused]
```

Restores a VM from a snapshot directory. The VM resumes automatically unless `--paused` is set.

Resource limits are validated against the current host before restore.

## version

```
$ microvm version
```
