use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "batuta",
    version = "0.1.0 (compozy floor v0.3.0-beta.16)",
    about = "Read-only terminal client for CompozyOS"
)]
pub struct Cli {
    #[arg(long, global = true, env = "COMPOZY_WORKSPACE")]
    pub workspace: Option<String>,
    #[arg(long, global = true, value_enum, default_value_t = DaemonArg::Auto)]
    pub daemon: DaemonArg,
    #[arg(long, global = true, default_value = "localhost:2123")]
    pub tcp_addr: String,
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
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
