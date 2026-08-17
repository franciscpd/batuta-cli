mod cli;
mod doctor;
mod exit;
mod logging;
mod relative_time;
mod sessions;
mod version;
mod workspace;

use clap::Parser;
use cli::{Cli, Command, DaemonArg};
use compozy_client::{Client, TransportOrder};
use exit::AppError;
use std::{path::PathBuf, time::Duration};

fn main() {
    let cli = Cli::parse();
    let logging = logging::init();
    let exit = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create Tokio runtime")
        .block_on(run(cli));
    drop(logging);
    std::process::exit(exit);
}

async fn run(cli: Cli) -> i32 {
    tokio::select! {
        result = run_command(cli) => result,
        _ = tokio::signal::ctrl_c() => interrupted_exit_code(),
    }
}

const fn interrupted_exit_code() -> i32 {
    130
}

async fn run_command(cli: Cli) -> i32 {
    tracing::debug!(command = ?cli.command, "batuta command");
    let result = match cli.command {
        Command::Doctor => doctor::run(&cli).await,
        Command::Sessions { all_agents, limit } => sessions::run(&cli, all_agents, limit).await,
        Command::Tail {
            session: _,
            all_agents: _,
        } => Err(AppError::unavailable("tail is not available yet")),
    };

    match result {
        Ok(()) => 0,
        Err(error) => {
            if !error.was_reported() {
                eprintln!("error: {error}");
            }
            error.exit_code()
        }
    }
}

pub(crate) fn transport_order(daemon: DaemonArg) -> TransportOrder {
    match daemon {
        DaemonArg::Auto => TransportOrder::Auto,
        DaemonArg::Uds => TransportOrder::UdsOnly,
        DaemonArg::Tcp => TransportOrder::TcpOnly,
    }
}

pub(crate) fn compozy_home() -> PathBuf {
    std::env::var_os("COMPOZY_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".compozy")))
        .unwrap_or_else(|| PathBuf::from(".compozy"))
}

pub(crate) async fn probe(cli: &Cli) -> (compozy_client::ProbeReport, Option<Client>) {
    Client::probe(
        transport_order(cli.daemon),
        compozy_home().join("daemon.sock"),
        &cli.tcp_addr,
        Duration::from_secs(3),
    )
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn ut_064_interrupt_has_standard_exit_code() {
        assert_eq!(super::interrupted_exit_code(), 130);
    }
}
