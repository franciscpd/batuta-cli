mod app;
mod cli;
mod config;
mod doctor;
mod exit;
mod logging;
mod relative_time;
mod sessions;
mod tail;
mod terminal;
mod version;
mod workspace;

use clap::Parser;
use cli::{Cli, Command, DaemonArg};
use compozy_client::{Client, TransportOrder};
use std::{path::PathBuf, time::Duration};

fn main() {
    let mut cli = Cli::parse();
    let settings = match config::load_and_resolve(&cli, &config::Env::current()) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };
    settings.apply_to_cli(&mut cli);
    let logging = logging::init();
    let exit = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create Tokio runtime")
        .block_on(run(cli, settings));
    drop(logging);
    std::process::exit(exit);
}

async fn run(cli: Cli, settings: config::Settings) -> i32 {
    tokio::select! {
        result = run_command(cli, settings) => result,
        _ = tokio::signal::ctrl_c() => interrupted_exit_code(),
    }
}

const fn interrupted_exit_code() -> i32 {
    130
}

async fn run_command(cli: Cli, settings: config::Settings) -> i32 {
    tracing::debug!(command = ?cli.command, "batuta command");
    let result = match cli.command {
        None => app::run(&cli, &settings).await,
        Some(Command::Doctor) => doctor::run(&cli, &settings).await,
        Some(Command::Sessions { all_agents, limit }) => {
            sessions::run(&cli, all_agents, limit).await
        }
        Some(Command::Tail {
            ref session,
            all_agents,
        }) => tail::run(&cli, &settings, session.as_deref(), all_agents).await,
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

pub(crate) fn transport_order(daemon: Option<DaemonArg>) -> TransportOrder {
    match daemon.unwrap_or(DaemonArg::Auto) {
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
        cli.tcp_addr.as_deref().unwrap_or("localhost:2123"),
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
