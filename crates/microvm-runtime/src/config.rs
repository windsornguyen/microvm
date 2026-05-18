use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub id: String,
    pub image: String,
    pub name: Option<String>,
    pub hostname: Option<String>,
    pub resources: Resources,
    pub process: ProcessConfig,
    pub mounts: Vec<MountConfig>,
    pub networks: Vec<String>,
    pub labels: Vec<(String, String)>,
    pub dns: Option<DnsConfig>,
    pub rosetta: bool,
    pub virtualization: bool,
    pub ssh: bool,
    pub use_init: bool,
    pub remove_on_exit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resources {
    pub cpus: u32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    pub executable: String,
    pub arguments: Vec<String>,
    pub environment: Vec<String>,
    pub working_directory: String,
    pub terminal: bool,
    pub user: UserConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserConfig {
    Name(String),
    Id { uid: u32, gid: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    pub source: PathBuf,
    pub destination: String,
    pub readonly: bool,
    pub mount_type: MountType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MountType {
    Bind,
    Volume(String),
    Tmpfs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub nameservers: Vec<String>,
    pub domain: Option<String>,
    pub search_domains: Vec<String>,
    pub options: Vec<String>,
}
