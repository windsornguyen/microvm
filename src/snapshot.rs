// Copyright (c) 2026 Windsor Nguyen. All rights reserved.

//! VM snapshot persistence: save and restore machine state to disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use microvm_vz::{DiskAttachment, FsShare, VmConfig, VmInstance};
use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;
const CONFIG_FILE: &str = "config.json";
const METADATA_FILE: &str = "metadata.json";
const MACHINE_ID_FILE: &str = "machine-id";
const MACHINE_STATE_FILE: &str = "machine-state";

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub config: SnapshotConfig,
    pub machine_state: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SnapshotConfig {
    pub cpus: u32,
    pub memory_bytes: u64,
    kernel: PathBuf,
    kernel_cmdline: Vec<String>,
    rootfs: PathBuf,
    disks: Vec<SnapshotDisk>,
    shares: Vec<SnapshotShare>,
    nested_virt: bool,
    machine_identifier_hex: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SnapshotDisk {
    path: PathBuf,
    serial: Option<String>,
    read_only: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SnapshotShare {
    tag: String,
    host_path: PathBuf,
    read_only: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SnapshotMetadata {
    schema_version: u32,
    microvm_version: String,
    created_unix_secs: u64,
    host_arch: String,
    host_os: String,
    config_file: String,
    machine_identifier_file: String,
    machine_state_file: String,
    resources: Vec<SnapshotResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SnapshotResource {
    kind: String,
    path: PathBuf,
    read_only: Option<bool>,
    size_bytes: Option<u64>,
    modified_unix_secs: Option<u64>,
}

impl SnapshotConfig {
    #[must_use]
    pub(crate) fn from_vm_config(config: &VmConfig) -> Self {
        Self {
            cpus: config.cpus,
            memory_bytes: config.memory_bytes,
            kernel: config.kernel.clone(),
            kernel_cmdline: config.kernel_cmdline.clone(),
            rootfs: config.rootfs.clone(),
            disks: config
                .disks
                .iter()
                .map(|disk| SnapshotDisk {
                    path: disk.path.clone(),
                    serial: disk.serial.clone(),
                    read_only: disk.read_only,
                })
                .collect(),
            shares: config
                .shares
                .iter()
                .map(|share| SnapshotShare {
                    tag: share.tag.clone(),
                    host_path: share.host_path.clone(),
                    read_only: share.read_only,
                })
                .collect(),
            nested_virt: config.nested_virt,
            machine_identifier_hex: hex_encode(config.machine_identifier()),
        }
    }

    pub(crate) fn to_vm_config(&self) -> Result<VmConfig> {
        Ok(VmConfig {
            cpus: self.cpus,
            memory_bytes: self.memory_bytes,
            kernel: self.kernel.clone(),
            kernel_cmdline: self.kernel_cmdline.clone(),
            rootfs: self.rootfs.clone(),
            disks: self
                .disks
                .iter()
                .map(|disk| DiskAttachment {
                    path: disk.path.clone(),
                    serial: disk.serial.clone(),
                    read_only: disk.read_only,
                })
                .collect(),
            shares: self
                .shares
                .iter()
                .map(|share| FsShare {
                    tag: share.tag.clone(),
                    host_path: share.host_path.clone(),
                    read_only: share.read_only,
                })
                .collect(),
            nested_virt: self.nested_virt,
            machine_identifier: Some(hex_decode(&self.machine_identifier_hex)?),
        })
    }
}

pub(crate) async fn write(path: &Path, vm: &mut VmInstance) -> Result<()> {
    ensure!(!path.exists(), "snapshot directory already exists: {}", path.display());
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("create snapshot parent {}", parent.display()))?;

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("snapshot path must have a UTF-8 directory name")?;
    let temp_path = parent.join(format!(".{name}.tmp.{}", std::process::id()));
    ensure!(
        !temp_path.exists(),
        "temporary snapshot directory already exists: {}",
        temp_path.display()
    );
    let mut temp = TempDir::create(temp_path)?;

    let config = SnapshotConfig::from_vm_config(vm.config());
    write_json(&temp.path.join(CONFIG_FILE), &config)?;
    fs::write(temp.path.join(MACHINE_ID_FILE), hex_decode(&config.machine_identifier_hex)?)
        .context("write machine identifier")?;

    let state_path = temp.path.join(MACHINE_STATE_FILE);
    vm.pause().await?;
    let result: Result<()> = async {
        vm.save_state(&state_path).await?;
        let metadata = SnapshotMetadata {
            schema_version: SCHEMA_VERSION,
            microvm_version: env!("CARGO_PKG_VERSION").to_owned(),
            created_unix_secs: unix_secs(SystemTime::now())?,
            host_arch: std::env::consts::ARCH.to_owned(),
            host_os: std::env::consts::OS.to_owned(),
            config_file: CONFIG_FILE.to_owned(),
            machine_identifier_file: MACHINE_ID_FILE.to_owned(),
            machine_state_file: MACHINE_STATE_FILE.to_owned(),
            resources: collect_resources(vm.config())?,
        };
        write_json(&temp.path.join(METADATA_FILE), &metadata)?;
        fs::rename(&temp.path, path)
            .with_context(|| format!("publish snapshot directory {}", path.display()))?;
        temp.persist();
        Ok(())
    }
    .await;

    let resume = vm.resume().await.context("resume VM after snapshot attempt");
    match (result, resume) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), Ok(())) | (Ok(()), Err(e)) => Err(e),
        (Err(snap_err), Err(resume_err)) => {
            Err(snap_err).context(format!("also failed to resume VM: {resume_err}"))
        }
    }
}

pub(crate) fn read(path: &Path) -> Result<Snapshot> {
    ensure!(path.is_dir(), "snapshot directory not found: {}", path.display());

    let metadata: SnapshotMetadata = read_json(&path.join(METADATA_FILE))?;
    ensure!(
        metadata.schema_version == SCHEMA_VERSION,
        "unsupported snapshot schema version: {}",
        metadata.schema_version
    );
    ensure!(
        metadata.config_file == CONFIG_FILE
            && metadata.machine_identifier_file == MACHINE_ID_FILE
            && metadata.machine_state_file == MACHINE_STATE_FILE,
        "snapshot metadata file names do not match microvm schema"
    );

    let config: SnapshotConfig = read_json(&path.join(CONFIG_FILE))?;
    let vm_config = config.to_vm_config()?;
    let machine_id = fs::read(path.join(MACHINE_ID_FILE)).context("read machine identifier")?;
    ensure!(
        hex_encode(&machine_id) == config.machine_identifier_hex,
        "snapshot machine-id does not match config.json"
    );

    for resource in &metadata.resources {
        resource.validate()?;
    }
    ensure!(
        metadata.resources == collect_resources(&vm_config)?,
        "snapshot external resources do not match config.json or host files"
    );

    let machine_state = path.join(MACHINE_STATE_FILE);
    ensure!(
        machine_state.metadata().map_or(0, |m| m.len()) > 0,
        "snapshot machine-state is missing or empty: {}",
        machine_state.display()
    );
    Ok(Snapshot { config, machine_state })
}

// --- helpers ---

fn collect_resources(config: &VmConfig) -> Result<Vec<SnapshotResource>> {
    let mut resources = vec![
        SnapshotResource::from_path("kernel", &config.kernel, None)?,
        SnapshotResource::from_path("rootfs", &config.rootfs, Some(false))?,
    ];
    for disk in &config.disks {
        resources.push(SnapshotResource::from_path("disk", &disk.path, Some(disk.read_only))?);
    }
    for share in &config.shares {
        resources.push(SnapshotResource::from_path(
            "share",
            &share.host_path,
            Some(share.read_only),
        )?);
    }
    Ok(resources)
}

impl SnapshotResource {
    fn from_path(kind: &str, path: &Path, read_only: Option<bool>) -> Result<Self> {
        let metadata =
            fs::metadata(path).with_context(|| format!("{kind} not found: {}", path.display()))?;
        Ok(Self {
            kind: kind.to_owned(),
            path: path.to_path_buf(),
            read_only,
            size_bytes: metadata.is_file().then_some(metadata.len()),
            modified_unix_secs: metadata.modified().ok().and_then(|t| unix_secs(t).ok()),
        })
    }

    fn validate(&self) -> Result<()> {
        let actual = Self::from_path(&self.kind, &self.path, self.read_only)?;
        ensure!(
            actual.size_bytes == self.size_bytes,
            "{} size changed: {}",
            self.kind,
            self.path.display()
        );
        ensure!(
            actual.modified_unix_secs == self.modified_unix_secs,
            "{} mtime changed: {}",
            self.kind,
            self.path.display()
        );
        Ok(())
    }
}

struct TempDir {
    path: PathBuf,
    persist: bool,
}

impl TempDir {
    fn create(path: PathBuf) -> Result<Self> {
        fs::create_dir(&path)
            .with_context(|| format!("create temporary snapshot directory {}", path.display()))?;
        Ok(Self { path, persist: false })
    }

    fn persist(&mut self) {
        self.persist = true;
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.persist {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_vec_pretty(value).context("serialize snapshot JSON")?;
    fs::write(path, [json, b"\n".to_vec()].concat())
        .with_context(|| format!("write {}", path.display()))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn unix_secs(time: SystemTime) -> Result<u64> {
    Ok(time.duration_since(UNIX_EPOCH)?.as_secs())
}

#[must_use]
fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        #[allow(clippy::expect_used)] // write to String is infallible
        write!(&mut out, "{byte:02x}").expect("write to String");
    }
    out
}

fn hex_decode(hex: &str) -> Result<Vec<u8>> {
    ensure!(hex.len().is_multiple_of(2), "hex string has odd length");
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => bail!("invalid hex digit: {}", byte as char),
    }
}

#[cfg(test)]
// Tests assert on success/failure; unwrap is the idiomatic assertion mechanism.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // --- snapshot config ---

    #[test]
    fn invariant_config_round_trips_machine_identifier() {
        let config = VmConfig {
            cpus: 2,
            memory_bytes: 512 * 1024 * 1024,
            kernel: PathBuf::from("vmlinuz"),
            kernel_cmdline: vec!["panic=1".to_owned()],
            rootfs: PathBuf::from("rootfs.ext4"),
            disks: vec![],
            shares: vec![],
            nested_virt: true,
            machine_identifier: Some(vec![1, 2, 3, 4]),
        };
        let snapshot = SnapshotConfig::from_vm_config(&config);
        let restored = snapshot.to_vm_config().unwrap();

        assert_eq!(restored.cpus, config.cpus);
        assert_eq!(restored.memory_bytes, config.memory_bytes);
        assert_eq!(restored.kernel_cmdline, config.kernel_cmdline);
        assert_eq!(restored.nested_virt, config.nested_virt);
        assert_eq!(restored.machine_identifier, config.machine_identifier);
    }

    // --- resource validation ---

    #[test]
    fn invariant_resource_rejects_file_drift() {
        let dir = test_dir();
        let rootfs = dir.join("rootfs.ext4");
        fs::write(&rootfs, b"before").unwrap();
        let resource = SnapshotResource::from_path("rootfs", &rootfs, Some(false)).unwrap();

        fs::write(&rootfs, b"after-drift").unwrap();

        let err = resource.validate().unwrap_err().to_string();
        assert!(err.contains("rootfs size changed"), "{err}");
    }

    // --- snapshot read validation ---

    #[test]
    fn invariant_read_rejects_machine_id_mismatch() {
        let dir = test_dir();
        let kernel = dir.join("vmlinuz");
        let rootfs = dir.join("rootfs.ext4");
        fs::write(&kernel, b"kernel").unwrap();
        fs::write(&rootfs, b"rootfs").unwrap();
        fs::write(dir.join(MACHINE_STATE_FILE), b"state").unwrap();
        fs::write(dir.join(MACHINE_ID_FILE), [9_u8]).unwrap();

        let vm_cfg = VmConfig {
            cpus: 1,
            memory_bytes: 128 * 1024 * 1024,
            kernel: kernel.clone(),
            kernel_cmdline: vec![],
            rootfs: rootfs.clone(),
            disks: vec![],
            shares: vec![],
            nested_virt: false,
            machine_identifier: Some(vec![1]),
        };
        let config = SnapshotConfig::from_vm_config(&vm_cfg);
        write_json(&dir.join(CONFIG_FILE), &config).unwrap();
        write_json(
            &dir.join(METADATA_FILE),
            &SnapshotMetadata {
                schema_version: SCHEMA_VERSION,
                microvm_version: env!("CARGO_PKG_VERSION").to_owned(),
                created_unix_secs: 1,
                host_arch: std::env::consts::ARCH.to_owned(),
                host_os: std::env::consts::OS.to_owned(),
                config_file: CONFIG_FILE.to_owned(),
                machine_identifier_file: MACHINE_ID_FILE.to_owned(),
                machine_state_file: MACHINE_STATE_FILE.to_owned(),
                resources: collect_resources(&config.to_vm_config().unwrap()).unwrap(),
            },
        )
        .unwrap();

        let err = read(&dir).unwrap_err().to_string();
        assert!(err.contains("machine-id does not match"), "{err}");
    }

    #[test]
    fn invariant_read_rejects_resource_mismatch() {
        let dir = test_dir();
        let kernel = dir.join("vmlinuz");
        let rootfs = dir.join("rootfs.ext4");
        fs::write(&kernel, b"kernel").unwrap();
        fs::write(&rootfs, b"rootfs").unwrap();
        fs::write(dir.join(MACHINE_STATE_FILE), b"state").unwrap();
        fs::write(dir.join(MACHINE_ID_FILE), [1_u8]).unwrap();

        let vm_cfg = VmConfig {
            cpus: 1,
            memory_bytes: 128 * 1024 * 1024,
            kernel,
            kernel_cmdline: vec![],
            rootfs,
            disks: vec![],
            shares: vec![],
            nested_virt: false,
            machine_identifier: Some(vec![1]),
        };
        let config = SnapshotConfig::from_vm_config(&vm_cfg);
        write_json(&dir.join(CONFIG_FILE), &config).unwrap();
        write_json(
            &dir.join(METADATA_FILE),
            &SnapshotMetadata {
                schema_version: SCHEMA_VERSION,
                microvm_version: env!("CARGO_PKG_VERSION").to_owned(),
                created_unix_secs: 1,
                host_arch: std::env::consts::ARCH.to_owned(),
                host_os: std::env::consts::OS.to_owned(),
                config_file: CONFIG_FILE.to_owned(),
                machine_identifier_file: MACHINE_ID_FILE.to_owned(),
                machine_state_file: MACHINE_STATE_FILE.to_owned(),
                resources: vec![],
            },
        )
        .unwrap();

        let err = read(&dir).unwrap_err().to_string();
        assert!(err.contains("external resources"), "{err}");
    }

    fn test_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "microvm-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
}
