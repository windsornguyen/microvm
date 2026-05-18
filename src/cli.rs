// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! Command-line entrypoints for the minimal VM runner.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use microvm_vz::{VmConfig, VmInstance};

pub enum Cli {
    Boot(BootArgs),
    Version,
}

pub struct BootArgs {
    kernel: PathBuf,
    rootfs: PathBuf,
    cmdline: Vec<String>,
    cpus: u32,
    memory: u32,
}

impl Cli {
    pub fn parse() -> Result<Self> {
        parse_args(env::args().skip(1))
    }

    pub async fn run(self) -> Result<()> {
        match self {
            Self::Boot(args) => boot(args).await,
            Self::Version => {
                println!("microvm {}", env!("CARGO_PKG_VERSION"));
                Ok(())
            }
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Cli> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("boot") => BootArgs::parse(args).map(Cli::Boot),
        Some("version" | "--version" | "-V") => Ok(Cli::Version),
        Some(command) => bail!("unknown command: {command}\n{}", usage()),
        None => bail!("{}", usage()),
    }
}

impl BootArgs {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut args = args.into_iter();
        let mut boot = BootBuilder::default();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-k" | "--kernel" => {
                    boot.kernel = Some(PathBuf::from(next_value(&mut args, &arg)?))
                }
                "-r" | "--rootfs" => {
                    boot.rootfs = Some(PathBuf::from(next_value(&mut args, &arg)?))
                }
                "--cmdline" => boot.cmdline.push(next_value(&mut args, &arg)?),
                "-c" | "--cpus" => boot.cpus = Some(parse_u32(&mut args, &arg)?),
                "-m" | "--memory" => boot.memory = Some(parse_u32(&mut args, &arg)?),
                _ => bail!("unknown boot argument: {arg}\n{}", usage()),
            }
        }

        Ok(Self {
            kernel: boot.kernel.context("missing --kernel")?,
            rootfs: boot.rootfs.context("missing --rootfs")?,
            cmdline: boot.cmdline,
            cpus: boot.cpus.unwrap_or(2),
            memory: boot.memory.unwrap_or(512),
        })
    }
}

#[derive(Default)]
struct BootBuilder {
    kernel: Option<PathBuf>,
    rootfs: Option<PathBuf>,
    cmdline: Vec<String>,
    cpus: Option<u32>,
    memory: Option<u32>,
}

async fn boot(args: BootArgs) -> Result<()> {
    let mut vm = VmInstance::new(VmConfig {
        cpus: args.cpus,
        memory_bytes: u64::from(args.memory) * 1024 * 1024,
        kernel: args.kernel,
        kernel_cmdline: kernel_cmdline(args.cmdline),
        rootfs: args.rootfs,
    })?;

    println!("booting: {} cpus, {} MiB", args.cpus, args.memory);
    vm.start().await?;
    println!("vm started, press ctrl-c to stop");

    tokio::signal::ctrl_c().await?;
    println!("stopping...");
    vm.stop().await?;
    println!("stopped");
    Ok(())
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .with_context(|| format!("missing value for {flag}"))
}

fn parse_u32(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<u32> {
    next_value(args, flag)?
        .parse()
        .with_context(|| format!("invalid value for {flag}"))
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

fn usage() -> &'static str {
    "usage: microvm boot --kernel <path> --rootfs <path> [--cmdline <arg>] [-c cpus] [-m mib]\n       microvm version"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_boot_command() {
        let cli = parse_args([
            "boot".to_owned(),
            "--kernel".to_owned(),
            "vmlinuz".to_owned(),
            "--rootfs".to_owned(),
            "rootfs.ext4".to_owned(),
            "--cmdline".to_owned(),
            "panic=1".to_owned(),
        ])
        .unwrap();

        let Cli::Boot(args) = cli else {
            panic!("expected boot command");
        };
        assert_eq!(args.kernel, PathBuf::from("vmlinuz"));
        assert_eq!(args.rootfs, PathBuf::from("rootfs.ext4"));
        assert_eq!(args.cmdline, vec!["panic=1"]);
        assert_eq!(args.cpus, 2);
        assert_eq!(args.memory, 512);
    }

    #[test]
    fn rejects_missing_rootfs() {
        assert!(
            parse_args([
                "boot".to_owned(),
                "--kernel".to_owned(),
                "vmlinuz".to_owned()
            ])
            .is_err()
        );
    }
}
