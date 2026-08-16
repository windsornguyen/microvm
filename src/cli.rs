// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Command-line entrypoints for the minimal VM runner.
// CLI binary: stdout/stderr output is the primary user interface.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Parser, Subcommand};
use microvm_vz::{VmConfig, VmInstance, VmPhase, VzError};

use crate::snapshot;

mod resource_limit {
    pub(super) const NUMERATOR: u64 = 3;
    pub(super) const DENOMINATOR: u64 = 4;
    pub(super) const PERCENT: u64 = 75;
}

mod shutdown {
    pub(super) const GUEST_TIMEOUT_SECS: u64 = 10;
}

#[derive(Parser)]
#[command(name = "microvm", version, about = "Lightweight macOS microVM runner")]
pub(crate) struct Cli {
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
}

#[derive(Args)]
struct RestoreArgs {
    #[arg(long)]
    from: PathBuf,
    #[arg(long)]
    paused: bool,
}

// --- public entrypoints ---

impl Cli {
    /// Run the selected command to completion.
    ///
    /// # Cancellation safety
    /// Not cancellation-safe while a mutating VZ operation is in flight.
    pub(crate) async fn run(self) -> Result<()> {
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
    let config = vm_config(
        args.cpus,
        args.memory,
        args.kernel,
        args.rootfs,
        args.cmdline,
        args.nested_virt,
        None,
    );
    let mut vm = create_vm(config).await?;

    println!("booting: {} cpus, {} MiB", args.cpus, args.memory);
    if args.snapshot.is_some() {
        vm.start_save_restore().await?;
    } else {
        vm.start().await?;
    }
    println!("vm started, press ctrl-c to stop");

    let run = async {
        if let Some(ref path) = args.snapshot {
            println!("saving snapshot to {}...", path.display());
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            snapshot::write(path, &mut vm).await?;
            println!("snapshot saved");
        }
        wait_for_vm(&mut vm).await
    }
    .await;
    finish_vm(&mut vm, run).await
}

async fn restore(args: RestoreArgs) -> Result<()> {
    let snap = snapshot::read(args.from.clone()).await?;
    let config = snap.config.to_vm_config()?;
    let mut vm = create_vm(config).await?;

    println!("restoring snapshot: {}", args.from.display());
    vm.restore(&snap.machine_state).await?;
    println!("vm restored: paused");
    let run = if args.paused {
        println!("vm paused, press ctrl-c to stop");
        tokio::signal::ctrl_c().await.map_err(anyhow::Error::from)
    } else {
        async {
            vm.resume().await?;
            println!("vm resumed, press ctrl-c to stop");
            wait_for_vm(&mut vm).await
        }
        .await
    };
    finish_vm(&mut vm, run).await
}

// --- helpers ---

async fn create_vm(config: VmConfig) -> Result<VmInstance> {
    tokio::task::spawn_blocking(move || {
        validate_resources(config.cpus, memory_mib(config.memory_bytes)?)?;
        VmInstance::new(config).map_err(anyhow::Error::from)
    })
    .await
    .context("VM configuration task failed")?
}

fn memory_mib(memory_bytes: u64) -> Result<u32> {
    const MIB: u64 = 1024 * 1024;

    u32::try_from(memory_bytes.div_ceil(MIB))
        .with_context(|| format!("snapshot memory does not fit CLI limits: {memory_bytes} bytes"))
}

async fn wait_for_vm(vm: &mut VmInstance) -> Result<()> {
    enum Wait {
        Guest(Result<(), VzError>),
        Signal(std::io::Result<()>),
    }

    // A terminal VZ event wins a simultaneous signal so lifecycle state is
    // consumed before any shutdown action is considered.
    let wait = tokio::select! {
        biased;
        result = vm.wait_for_stop() => Wait::Guest(result),
        result = tokio::signal::ctrl_c() => Wait::Signal(result),
    };
    match wait {
        Wait::Guest(result) => {
            result?;
            println!("vm stopped");
            Ok(())
        }
        Wait::Signal(result) => {
            result?;
            shutdown_vm(vm).await
        }
    }
}

async fn shutdown_vm(vm: &mut VmInstance) -> Result<()> {
    if vm.phase() == VmPhase::Paused {
        println!("vm is paused; force-stopping...");
        return force_stop_vm(vm).await;
    }

    println!("requesting guest shutdown...");
    if let Err(request_error) = vm.request_stop().await {
        return match force_stop_vm(vm).await {
            Ok(()) => Err(request_error).context("guest shutdown request failed; VM force-stopped"),
            Err(stop_error) => Err(request_error).context(format!(
                "guest shutdown request failed; force-stop also failed: {stop_error}"
            )),
        };
    }
    if vm.phase() == VmPhase::Stopped {
        println!("stopped");
        return Ok(());
    }

    let timeout = std::time::Duration::from_secs(shutdown::GUEST_TIMEOUT_SECS);
    if let Ok(result) = tokio::time::timeout(timeout, vm.wait_for_stop()).await {
        result?;
        println!("stopped");
        Ok(())
    } else {
        eprintln!(
            "guest did not stop after {}s; force-stopping the VM",
            shutdown::GUEST_TIMEOUT_SECS
        );
        force_stop_vm(vm).await
    }
}

async fn force_stop_vm(vm: &mut VmInstance) -> Result<()> {
    if vm.phase() == VmPhase::Stopped {
        return Ok(());
    }
    if let Some(result) = vm.try_finish_stop() {
        result?;
        println!("stopped");
        return Ok(());
    }

    vm.stop().await?;
    println!("stopped");
    Ok(())
}

async fn finish_vm(vm: &mut VmInstance, run: Result<()>) -> Result<()> {
    let cleanup = if vm.phase() == VmPhase::Stopped { Ok(()) } else { force_stop_vm(vm).await };
    match (run, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(run_error), Err(cleanup_error)) => {
            Err(run_error).context(format!("VM cleanup also failed: {cleanup_error}"))
        }
    }
}

fn validate_resources(cpus: u32, memory_mib: u32) -> Result<()> {
    let host_mem = host_memory_bytes()?;
    let requested = u64::from(memory_mib) * 1024 * 1024;
    let limit = host_mem
        .checked_mul(resource_limit::NUMERATOR)
        .context("host memory overflows resource-limit calculation")?
        / resource_limit::DENOMINATOR;
    let host_gib = host_mem / (1024 * 1024 * 1024);
    let limit_mib = limit / (1024 * 1024);
    ensure!(
        requested <= limit,
        "requested {memory_mib} MiB exceeds {}% of host memory \
         ({host_gib} GiB, limit {limit_mib} MiB)",
        resource_limit::PERCENT,
    );

    let host_cpus = host_cpu_count()?;
    let limit =
        (u64::from(host_cpus) * resource_limit::NUMERATOR).div_ceil(resource_limit::DENOMINATOR);
    ensure!(
        u64::from(cpus) <= limit,
        "requested {cpus} vCPUs exceeds {}% of host CPUs \
         ({host_cpus} cores, limit {limit})",
        resource_limit::PERCENT,
    );
    Ok(())
}

fn host_memory_bytes() -> Result<u64> {
    use sysctl::Sysctl;
    let ctl = sysctl::Ctl::new("hw.memsize").context("open sysctl hw.memsize")?;
    host_memory_from_sysctl(&ctl.value().context("read sysctl hw.memsize")?)
}

fn host_memory_from_sysctl(value: &sysctl::CtlValue) -> Result<u64> {
    match value {
        sysctl::CtlValue::U64(value) => Ok(*value),
        sysctl::CtlValue::S64(value) => {
            u64::try_from(*value).context("sysctl hw.memsize returned a negative value")
        }
        _ => bail!("sysctl hw.memsize returned an unexpected value type"),
    }
}

fn host_cpu_count() -> Result<u32> {
    use sysctl::Sysctl;
    let ctl = sysctl::Ctl::new("hw.activecpu").context("open sysctl hw.activecpu")?;
    host_cpu_count_from_sysctl(&ctl.value().context("read sysctl hw.activecpu")?)
}

fn host_cpu_count_from_sysctl(value: &sysctl::CtlValue) -> Result<u32> {
    match value {
        sysctl::CtlValue::Int(value) => {
            u32::try_from(*value).context("sysctl hw.activecpu returned a negative value")
        }
        sysctl::CtlValue::U32(value) => Ok(*value),
        _ => bail!("sysctl hw.activecpu returned an unexpected value type"),
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
// Tests assert on success/failure; unwrap is the idiomatic assertion mechanism.
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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
        let host_mib = u32::try_from(host_mem / (1024 * 1024)).unwrap();

        assert!(validate_resources(2, 512).is_ok());
        assert!(validate_resources(2, host_mib).is_err());
    }

    #[test]
    fn invariant_safe_memory_accepted() {
        let host_mem = host_memory_bytes().expect("sysctl hw.memsize");
        let under = host_mem * resource_limit::NUMERATOR / resource_limit::DENOMINATOR / 2;
        let under_mib = u32::try_from(under / (1024 * 1024)).unwrap();
        assert!(validate_resources(2, under_mib).is_ok());
    }

    #[test]
    fn invariant_excessive_cpus_rejected() {
        let host_cpus = host_cpu_count().expect("sysctl hw.ncpu");
        let err = validate_resources(host_cpus + 1, 512).unwrap_err().to_string();
        assert!(err.contains("vCPUs"), "{err}");
    }

    #[test]
    fn invariant_safe_cpus_accepted() {
        let host_cpus = host_cpu_count().expect("sysctl hw.activecpu");
        let limit = (u64::from(host_cpus) * resource_limit::NUMERATOR)
            .div_ceil(resource_limit::DENOMINATOR);
        assert!(validate_resources(u32::try_from(limit).unwrap(), 512).is_ok());
    }

    #[test]
    fn invariant_partial_memory_mib_rounds_up() {
        assert_eq!(memory_mib(1024 * 1024 + 1).unwrap(), 2);
    }

    #[test]
    fn invariant_unexpected_host_memory_type_rejected() {
        let value = sysctl::CtlValue::String("unknown".to_owned());
        assert!(host_memory_from_sysctl(&value).is_err());
    }

    #[test]
    fn invariant_unexpected_host_cpu_type_rejected() {
        let value = sysctl::CtlValue::String("unknown".to_owned());
        assert!(host_cpu_count_from_sysctl(&value).is_err());
    }
}
