// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Command-line entrypoints for the minimal VM runner.

use std::path::PathBuf;

use anyhow::{Result, ensure};
use clap::{Args, Parser, Subcommand};
use microvm_vz::{VmConfig, VmInstance};

use crate::snapshot;

const MAX_MEMORY_HOST_FRACTION: f64 = 0.75;
const MAX_CPUS_HOST_FRACTION: f64 = 0.75;
const STOP_TIMEOUT_SECS: u64 = 10;

#[derive(Parser)]
#[command(name = "microvm", version, about = "Lightweight macOS microVM runner")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Boot(BootArgs),
    Restore(RestoreArgs),
    Version,
}

#[derive(Args)]
struct BootArgs {
    #[arg(short, long)]
    kernel: PathBuf,
    #[arg(short, long)]
    rootfs: PathBuf,
    #[arg(long)]
    cmdline: Vec<String>,
    #[arg(short, long, default_value_t = 2)]
    cpus: u32,
    #[arg(short, long, default_value_t = 512)]
    memory: u32,
    #[arg(long)]
    nested_virt: bool,
    #[arg(long, alias = "checkpoint")]
    snapshot: Option<PathBuf>,
    /// Bypass host resource safety checks.
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct RestoreArgs {
    #[arg(long)]
    from: PathBuf,
    #[arg(long)]
    paused: bool,
    /// Bypass host resource safety checks.
    #[arg(long)]
    force: bool,
}

// --- public entrypoints ---

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Boot(args) => boot(args).await,
            Command::Restore(args) => restore(args).await,
            Command::Version => {
                println!("microvm {}", env!("CARGO_PKG_VERSION"));
                Ok(())
            }
        }
    }
}

async fn boot(args: BootArgs) -> Result<()> {
    if !args.force {
        validate_resources(args.cpus, args.memory)?;
    }

    let mut vm = VmInstance::new(vm_config(
        args.cpus,
        args.memory,
        args.kernel,
        args.rootfs,
        args.cmdline,
        args.nested_virt,
        None,
    ))?;

    println!("booting: {} cpus, {} MiB", args.cpus, args.memory);
    if args.snapshot.is_some() {
        vm.start_save_restore().await?;
    } else {
        vm.start().await?;
    }
    println!("vm started, press ctrl-c to stop");

    if let Some(ref path) = args.snapshot {
        println!("saving snapshot to {}...", path.display());
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        snapshot::write(path, &mut vm).await?;
        println!("snapshot saved");
    }

    tokio::signal::ctrl_c().await?;
    stop_vm(&mut vm).await;
    Ok(())
}

async fn restore(args: RestoreArgs) -> Result<()> {
    let snap = snapshot::read(&args.from)?;
    if !args.force {
        // Snapshot memory is validated against host limits; values above u32::MAX MiB
        // (4 PiB) are not physically possible.
        #[allow(clippy::cast_possible_truncation)]
        let mem_mib = (snap.config.memory_bytes / (1024 * 1024)) as u32;
        validate_resources(snap.config.cpus, mem_mib)?;
    }
    let mut vm = VmInstance::new(snap.config.to_vm_config()?)?;

    println!("restoring snapshot: {}", args.from.display());
    vm.restore(&snap.machine_state).await?;
    println!("vm restored: paused");
    if args.paused {
        println!("vm paused, press ctrl-c to stop");
    } else {
        vm.resume().await?;
        println!("vm resumed, press ctrl-c to stop");
    }

    tokio::signal::ctrl_c().await?;
    stop_vm(&mut vm).await;
    Ok(())
}

// --- helpers ---

async fn stop_vm(vm: &mut VmInstance) {
    println!("stopping...");
    let timeout = std::time::Duration::from_secs(STOP_TIMEOUT_SECS);
    match tokio::time::timeout(timeout, vm.stop()).await {
        Ok(Ok(())) => println!("stopped"),
        Ok(Err(e)) => eprintln!("warning: stop failed: {e}"),
        Err(_) => eprintln!("warning: stop timed out after {STOP_TIMEOUT_SECS}s, forcing exit"),
    }
}

// Percentage-of-host arithmetic: values are bounded by physical hardware
// (no host exceeds u32::MAX MiB or u32::MAX cores).
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn validate_resources(cpus: u32, memory_mib: u32) -> Result<()> {
    if let Some(host_mem) = host_memory_bytes() {
        let requested = u64::from(memory_mib) * 1024 * 1024;
        let limit = (host_mem as f64 * MAX_MEMORY_HOST_FRACTION) as u64;
        let host_gib = host_mem / (1024 * 1024 * 1024);
        let limit_mib = limit / (1024 * 1024);
        ensure!(
            requested <= limit,
            "requested {memory_mib} MiB exceeds {:.0}% of host memory \
             ({host_gib} GiB, limit {limit_mib} MiB). \
             Use --force to override.",
            MAX_MEMORY_HOST_FRACTION * 100.0,
        );
    }
    if let Some(host_cpus) = host_cpu_count() {
        let limit = (f64::from(host_cpus) * MAX_CPUS_HOST_FRACTION).ceil() as u32;
        ensure!(
            cpus <= limit,
            "requested {cpus} vCPUs exceeds {:.0}% of host CPUs \
             ({host_cpus} cores, limit {limit}). \
             Use --force to override.",
            MAX_CPUS_HOST_FRACTION * 100.0,
        );
    }
    Ok(())
}

fn host_memory_bytes() -> Option<u64> {
    use sysctl::Sysctl;
    let ctl = sysctl::Ctl::new("hw.memsize").ok()?;
    match ctl.value().ok()? {
        sysctl::CtlValue::U64(v) => Some(v),
        sysctl::CtlValue::S64(v) => u64::try_from(v).ok(),
        _ => None,
    }
}

fn host_cpu_count() -> Option<u32> {
    use sysctl::Sysctl;
    let ctl = sysctl::Ctl::new("hw.ncpu").ok()?;
    match ctl.value().ok()? {
        sysctl::CtlValue::Int(v) => u32::try_from(v).ok(),
        sysctl::CtlValue::U32(v) => Some(v),
        _ => None,
    }
}

#[must_use]
fn vm_config(
    cpus: u32,
    memory_mib: u32,
    kernel: PathBuf,
    rootfs: PathBuf,
    cmdline: Vec<String>,
    nested_virt: bool,
    machine_identifier: Option<Vec<u8>>,
) -> VmConfig {
    VmConfig {
        cpus,
        memory_bytes: u64::from(memory_mib) * 1024 * 1024,
        kernel,
        kernel_cmdline: kernel_cmdline(cmdline),
        rootfs,
        disks: vec![],
        shares: vec![],
        nested_virt,
        machine_identifier,
    }
}

#[must_use]
fn kernel_cmdline(extra: Vec<String>) -> Vec<String> {
    [
        "console=hvc0".to_owned(),
        "root=/dev/vda".to_owned(),
        "rootfstype=ext4".to_owned(),
        "rw".to_owned(),
        "init=/bin/sh".to_owned(),
    ]
    .into_iter()
    .chain(extra)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // --- CLI parsing ---

    #[test]
    fn invariant_boot_parses_defaults() {
        let cli = Cli::try_parse_from([
            "microvm",
            "boot",
            "--kernel",
            "vmlinuz",
            "--rootfs",
            "rootfs.ext4",
            "--cmdline",
            "panic=1",
        ])
        .unwrap();

        let Command::Boot(args) = cli.command else { panic!("expected boot") };
        assert_eq!(args.kernel, PathBuf::from("vmlinuz"));
        assert_eq!(args.rootfs, PathBuf::from("rootfs.ext4"));
        assert_eq!(args.cmdline, vec!["panic=1"]);
        assert_eq!(args.cpus, 2);
        assert_eq!(args.memory, 512);
        assert!(!args.nested_virt);
        assert!(!args.force);
    }

    #[test]
    fn invariant_nested_virt_flag_parses() {
        let cli = Cli::try_parse_from([
            "microvm",
            "boot",
            "--kernel",
            "vmlinuz",
            "--rootfs",
            "rootfs.ext4",
            "--nested-virt",
        ])
        .unwrap();
        let Command::Boot(args) = cli.command else { panic!("expected boot") };
        assert!(args.nested_virt);
    }

    #[test]
    fn invariant_snapshot_and_restore_parse() {
        let cli = Cli::try_parse_from([
            "microvm",
            "boot",
            "--kernel",
            "vmlinuz",
            "--rootfs",
            "rootfs.ext4",
            "--snapshot",
            "snap",
        ])
        .unwrap();
        let Command::Boot(args) = cli.command else { panic!("expected boot") };
        assert_eq!(args.snapshot, Some(PathBuf::from("snap")));

        let cli = Cli::try_parse_from(["microvm", "restore", "--from", "snap"]).unwrap();
        let Command::Restore(args) = cli.command else { panic!("expected restore") };
        assert_eq!(args.from, PathBuf::from("snap"));
        assert!(!args.paused);
    }

    #[test]
    fn invariant_force_flag_parses() {
        let cli = Cli::try_parse_from([
            "microvm",
            "boot",
            "--kernel",
            "vmlinuz",
            "--rootfs",
            "rootfs.ext4",
            "--memory",
            "999999",
            "--force",
        ])
        .unwrap();
        let Command::Boot(args) = cli.command else { panic!("expected boot") };
        assert!(args.force);
        assert_eq!(args.memory, 999_999);
    }

    #[test]
    fn design_old_virtualization_flag_rejected() {
        assert!(
            Cli::try_parse_from([
                "microvm",
                "boot",
                "--kernel",
                "vmlinuz",
                "--rootfs",
                "rootfs.ext4",
                "--virtualization",
            ])
            .is_err()
        );
    }

    #[test]
    fn invariant_missing_rootfs_rejected() {
        assert!(Cli::try_parse_from(["microvm", "boot", "--kernel", "vmlinuz"]).is_err());
    }

    // --- resource validation ---

    #[test]
    fn invariant_excessive_memory_rejected() {
        let host_mem = host_memory_bytes().expect("sysctl hw.memsize");
        let host_mib = (host_mem / (1024 * 1024)) as u32;

        assert!(validate_resources(2, 512).is_ok());
        assert!(validate_resources(2, host_mib).is_err());

        let just_over = (host_mem as f64 * MAX_MEMORY_HOST_FRACTION) as u64;
        let just_over_mib = (just_over / (1024 * 1024)) as u32 + 1;
        let err = validate_resources(2, just_over_mib).unwrap_err().to_string();
        assert!(err.contains("--force"), "{err}");
    }

    #[test]
    fn invariant_safe_memory_accepted() {
        let host_mem = host_memory_bytes().expect("sysctl hw.memsize");
        let under = (host_mem as f64 * MAX_MEMORY_HOST_FRACTION * 0.5) as u64;
        let under_mib = (under / (1024 * 1024)) as u32;
        assert!(validate_resources(2, under_mib).is_ok());
    }

    #[test]
    fn invariant_excessive_cpus_rejected() {
        let host_cpus = host_cpu_count().expect("sysctl hw.ncpu");
        let err = validate_resources(host_cpus + 1, 512).unwrap_err().to_string();
        assert!(err.contains("vCPUs"), "{err}");
        assert!(err.contains("--force"), "{err}");
    }

    #[test]
    fn invariant_safe_cpus_accepted() {
        let host_cpus = host_cpu_count().expect("sysctl hw.ncpu");
        let limit = (f64::from(host_cpus) * MAX_CPUS_HOST_FRACTION).ceil() as u32;
        assert!(validate_resources(limit, 512).is_ok());
    }
}
