use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "microvm", about = "A container runtime for macOS")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run(RunArgs),
    Create(CreateArgs),
    Start { container_id: String },
    Stop { container_ids: Vec<String> },
    #[command(alias = "rm")]
    Delete { container_ids: Vec<String> },
    Exec(ExecArgs),
    #[command(alias = "ls")]
    List,
    Inspect { container_ids: Vec<String> },
    Kill { container_ids: Vec<String> },
    Logs { container_id: String },
    Stats { container_ids: Vec<String> },
    Prune,
    Export { container_id: String },
    Checkpoint(CheckpointArgs),
    Restore(RestoreArgs),
    #[command(subcommand)]
    Image(ImageCommand),
    #[command(subcommand, alias = "v")]
    Volume(VolumeCommand),
    #[command(subcommand, alias = "n")]
    Network(NetworkCommand),
    #[command(subcommand, alias = "r")]
    Registry(RegistryCommand),
    #[command(subcommand, alias = "s")]
    System(SystemCommand),
    Build(BuildArgs),
}

#[derive(clap::Args)]
struct RunArgs {
    #[arg(short, long)]
    name: Option<String>,
    #[arg(short = 'd', long)]
    detach: bool,
    #[arg(short, long)]
    interactive: bool,
    #[arg(short = 't', long)]
    tty: bool,
    #[arg(long)]
    rm: bool,
    #[arg(short, long)]
    env: Vec<String>,
    #[arg(short, long)]
    cpus: Option<u32>,
    #[arg(short, long)]
    memory: Option<String>,
    #[arg(short = 'p', long)]
    publish: Vec<String>,
    #[arg(short = 'v', long)]
    volume: Vec<String>,
    #[arg(long)]
    mount: Vec<String>,
    #[arg(long)]
    network: Option<String>,
    #[arg(long)]
    virtualization: bool,
    #[arg(long)]
    rosetta: bool,
    #[arg(long)]
    ssh: bool,
    image: String,
    arguments: Vec<String>,
}

#[derive(clap::Args)]
struct CreateArgs {
    #[arg(short, long)]
    name: Option<String>,
    image: String,
    arguments: Vec<String>,
}

#[derive(clap::Args)]
struct ExecArgs {
    #[arg(short, long)]
    interactive: bool,
    #[arg(short = 't', long)]
    tty: bool,
    #[arg(short, long)]
    detach: bool,
    container_id: String,
    arguments: Vec<String>,
}

#[derive(clap::Args)]
struct CheckpointArgs {
    #[arg(short, long)]
    output: Option<String>,
    container_id: String,
}

#[derive(clap::Args)]
struct RestoreArgs {
    #[arg(short, long)]
    input: Option<String>,
    container_id: String,
}

#[derive(clap::Args)]
struct BuildArgs {
    #[arg(short = 'f', long)]
    file: Option<String>,
    #[arg(short = 't', long)]
    tag: Vec<String>,
    #[arg(long)]
    build_arg: Vec<String>,
    #[arg(long)]
    no_cache: bool,
    context: Option<String>,
}

#[derive(Subcommand)]
enum ImageCommand {
    #[command(alias = "ls")]
    List,
    Pull { reference: String },
    Push { reference: String },
    Inspect { images: Vec<String> },
    #[command(alias = "rm")]
    Delete { images: Vec<String> },
    Tag { source: String, target: String },
    Load,
    Save { references: Vec<String> },
    Prune,
}

#[derive(Subcommand)]
enum VolumeCommand {
    Create { name: String },
    #[command(alias = "ls")]
    List,
    Inspect { names: Vec<String> },
    #[command(alias = "rm")]
    Delete { names: Vec<String> },
    Prune,
}

#[derive(Subcommand)]
enum NetworkCommand {
    Create { name: String },
    #[command(alias = "ls")]
    List,
    Inspect { names: Vec<String> },
    #[command(alias = "rm")]
    Delete { names: Vec<String> },
    Prune,
}

#[derive(Subcommand)]
enum RegistryCommand {
    Login { server: String },
    Logout { server: String },
    #[command(alias = "ls")]
    List,
}

#[derive(Subcommand)]
enum SystemCommand {
    Start,
    Stop,
    Status,
    Df,
    Logs,
    Version,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Run(_args) => todo!("run"),
            Command::Create(_args) => todo!("create"),
            Command::Start { .. } => todo!("start"),
            Command::Stop { .. } => todo!("stop"),
            Command::Delete { .. } => todo!("delete"),
            Command::Exec(_args) => todo!("exec"),
            Command::List => todo!("list"),
            Command::Inspect { .. } => todo!("inspect"),
            Command::Kill { .. } => todo!("kill"),
            Command::Logs { .. } => todo!("logs"),
            Command::Stats { .. } => todo!("stats"),
            Command::Prune => todo!("prune"),
            Command::Export { .. } => todo!("export"),
            Command::Checkpoint(_args) => todo!("checkpoint"),
            Command::Restore(_args) => todo!("restore"),
            Command::Image(cmd) => match cmd {
                ImageCommand::List => todo!("image list"),
                ImageCommand::Pull { .. } => todo!("image pull"),
                ImageCommand::Push { .. } => todo!("image push"),
                ImageCommand::Inspect { .. } => todo!("image inspect"),
                ImageCommand::Delete { .. } => todo!("image delete"),
                ImageCommand::Tag { .. } => todo!("image tag"),
                ImageCommand::Load => todo!("image load"),
                ImageCommand::Save { .. } => todo!("image save"),
                ImageCommand::Prune => todo!("image prune"),
            },
            Command::Volume(cmd) => match cmd {
                VolumeCommand::Create { .. } => todo!("volume create"),
                VolumeCommand::List => todo!("volume list"),
                VolumeCommand::Inspect { .. } => todo!("volume inspect"),
                VolumeCommand::Delete { .. } => todo!("volume delete"),
                VolumeCommand::Prune => todo!("volume prune"),
            },
            Command::Network(cmd) => match cmd {
                NetworkCommand::Create { .. } => todo!("network create"),
                NetworkCommand::List => todo!("network list"),
                NetworkCommand::Inspect { .. } => todo!("network inspect"),
                NetworkCommand::Delete { .. } => todo!("network delete"),
                NetworkCommand::Prune => todo!("network prune"),
            },
            Command::Registry(cmd) => match cmd {
                RegistryCommand::Login { .. } => todo!("registry login"),
                RegistryCommand::Logout { .. } => todo!("registry logout"),
                RegistryCommand::List => todo!("registry list"),
            },
            Command::System(cmd) => match cmd {
                SystemCommand::Start => todo!("system start"),
                SystemCommand::Stop => todo!("system stop"),
                SystemCommand::Status => todo!("system status"),
                SystemCommand::Df => todo!("system df"),
                SystemCommand::Logs => todo!("system logs"),
                SystemCommand::Version => {
                    println!("microvm {}", env!("CARGO_PKG_VERSION"));
                    Ok(())
                }
            },
            Command::Build(_args) => todo!("build"),
        }
    }
}
