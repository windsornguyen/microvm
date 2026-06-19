// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Command-line entrypoints for the minimal VM runner.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use microvm_vz::{VmConfig, VmInstance};

#[derive(Parser)]
#[command(name = "microvm", version, about = "Lightweight macOS microVM runner")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Boot(BootArgs),
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
    /// Expose nested virtualization (/dev/kvm) to the guest.
    #[arg(long)]
    nested_virt: bool,
    /// Save a VM checkpoint after boot (golden snapshot).
    #[arg(long)]
    checkpoint: Option<PathBuf>,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Boot(args) => boot(args).await,
            Command::Version => {
                println!("microvm {}", env!("CARGO_PKG_VERSION"));
                Ok(())
            }
        }
    }
}

async fn boot(args: BootArgs) -> Result<()> {
    let mut vm = VmInstance::new(VmConfig {
        cpus: args.cpus,
        memory_bytes: u64::from(args.memory) * 1024 * 1024,
        kernel: args.kernel,
        kernel_cmdline: kernel_cmdline(args.cmdline),
        rootfs: args.rootfs,
        disks: vec![],
        shares: vec![],
        nested_virt: args.nested_virt,
    })?;

    println!("booting: {} cpus, {} MiB", args.cpus, args.memory);
    vm.start().await?;
    println!("vm started, press ctrl-c to stop");

    if let Some(ref path) = args.checkpoint {
        println!("saving checkpoint to {}...", path.display());
        // Wait a beat for the VM to stabilize before snapshotting.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        vm.checkpoint(path).await?;
        println!("checkpoint saved");
    }

    tokio::signal::ctrl_c().await?;
    println!("stopping...");
    vm.stop().await?;
    println!("stopped");
    Ok(())
}

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

    #[test]
    fn parses_boot_command() {
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

        let Command::Boot(args) = cli.command else {
            panic!("expected boot command");
        };
        assert_eq!(args.kernel, PathBuf::from("vmlinuz"));
        assert_eq!(args.rootfs, PathBuf::from("rootfs.ext4"));
        assert_eq!(args.cmdline, vec!["panic=1"]);
        assert_eq!(args.cpus, 2);
        assert_eq!(args.memory, 512);
        assert!(!args.nested_virt);
    }

    #[test]
    fn parses_nested_virt_flag() {
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

        let Command::Boot(args) = cli.command else {
            panic!("expected boot command");
        };
        assert!(args.nested_virt);
    }

    #[test]
    fn rejects_old_virtualization_flag() {
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
    fn rejects_missing_rootfs() {
        assert!(Cli::try_parse_from(["microvm", "boot", "--kernel", "vmlinuz"]).is_err());
    }
}
