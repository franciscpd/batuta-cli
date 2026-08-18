use crate::version;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "batuta",
    version = version::CLAP_VERSION,
    about = "Read-only terminal client for CompozyOS"
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub workspace: Option<String>,
    #[arg(long, global = true, value_enum)]
    pub daemon: Option<DaemonArg>,
    #[arg(long, global = true)]
    pub tcp_addr: Option<String>,
    #[arg(long, global = true)]
    pub config: Option<std::path::PathBuf>,
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum DaemonArg {
    Auto,
    Uds,
    Tcp,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Doctor,
    Sessions {
        #[arg(long)]
        all_agents: bool,
        #[arg(long, default_value_t = 20, allow_hyphen_values = true)]
        limit: i64,
    },
    Tail {
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        all_agents: bool,
    },
}
