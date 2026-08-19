use crate::{cli::Cli, config::Settings, exit::AppError, probe, terminal, version, workspace};
use batuta_tui::app::{AppMode, Model, Toast, ToastKind, WorkspaceRef};
use compozy_client::{Client, Outcome, ProbeReport, Transport};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use ratatui::{Frame, Terminal, backend::Backend, widgets::Paragraph};
use std::{io::IsTerminal, time::Duration};

pub async fn run(cli: &Cli, settings: &Settings) -> Result<(), AppError> {
    if !std::io::stdout().is_terminal() {
        return Err(AppError::daemon(tty_required_error()));
    }
    terminal::install_panic_hook();
    let mut terminal = ratatui::init();
    let _guard = terminal::TerminalGuard::enter(terminal::RatatuiOps)
        .map_err(|error| AppError::daemon(format!("initialize terminal: {error}")))?;
    let (client, status) = await_daemon(cli, &mut terminal).await?;
    let warning = version::check(status.daemon.version.as_deref());
    let workspace =
        match workspace::resolve_from_daemon(&client, settings.workspace.as_deref()).await {
            Ok(workspace) => Some(workspace),
            Err(error)
                if settings.workspace.is_none()
                    && error.to_string().starts_with("no workspace contains") =>
            {
                None
            }
            Err(error) => return Err(error),
        };
    let workspace_ref = workspace.as_ref().map(|workspace| WorkspaceRef {
        id: workspace.id.clone(),
        name: workspace.name.clone(),
        root_dir: workspace.root_dir.clone(),
    });
    let mut model = Model::new(settings.tui_settings(workspace_ref), AppMode::Full);
    model.daemon.status = status.daemon.status;
    model.daemon.version = status.daemon.version;
    if let Some(warning) = warning {
        model.toast = Some(Toast {
            kind: ToastKind::Info,
            text: warning,
            sticky: true,
        });
    }
    if let Some(warning) = settings.warnings.first() {
        model.toast = Some(Toast {
            kind: ToastKind::Info,
            text: format!("warning: {warning}"),
            sticky: true,
        });
    }
    run_model_in_terminal(model, client, &mut terminal).await
}

pub async fn run_model(model: Model, client: compozy_client::Client) -> Result<(), AppError> {
    terminal::install_panic_hook();
    let mut terminal = ratatui::init();
    let _guard = terminal::TerminalGuard::enter(terminal::RatatuiOps)
        .map_err(|error| AppError::daemon(format!("initialize terminal: {error}")))?;
    run_model_in_terminal(model, client, &mut terminal).await
}

async fn run_model_in_terminal<B>(
    model: Model,
    client: Client,
    terminal: &mut Terminal<B>,
) -> Result<(), AppError>
where
    B: Backend,
    B::Error: std::fmt::Display,
{
    #[cfg(unix)]
    let result = tokio::select! {
        result = batuta_tui::runtime::run(model, client, terminal) => result,
        _ = termination_signal() => Ok(()),
    };
    #[cfg(not(unix))]
    let result = batuta_tui::runtime::run(model, client, &mut terminal).await;
    result.map_err(AppError::daemon)
}

async fn await_daemon<B>(
    cli: &Cli,
    terminal: &mut Terminal<B>,
) -> Result<(Client, compozy_client::types::StatusPayload), AppError>
where
    B: Backend,
    B::Error: std::fmt::Display,
{
    let mut attempts = 0;
    let mut events = EventStream::new();
    loop {
        attempts += 1;
        tokio::select! {
            biased;
            quit = wait_for_quit(&mut events) => {
                if quit {
                    return Err(retry_quit());
                }
            }
            (report, client) = probe(cli) => {
                let last_error = match client {
                    Some(client) => tokio::select! {
                        biased;
                        quit = wait_for_quit(&mut events) => {
                            if quit {
                                return Err(retry_quit());
                            }
                            "terminal input closed during startup".into()
                        }
                        status = client.status() => match status {
                            Ok(status) => match startup_version_mismatch(&status) {
                                Some(error) => error,
                                None => return Ok((client, status)),
                            },
                            Err(error) => format!("connection lost during startup — {error}"),
                        },
                    },
                    None => last_probe_error(&report),
                };
                terminal
                    .draw(|frame| render_retry_screen(frame, attempts, &last_error))
                    .map_err(|error| AppError::daemon(format!("render retry screen: {error}")))?;
                if wait_or_quit(&mut events).await {
                    return Err(retry_quit());
                }
            }
        }
    }
}

fn startup_version_mismatch(status: &compozy_client::types::StatusPayload) -> Option<String> {
    let value = status.daemon.version.as_deref()?.trim();
    let daemon = semver::Version::parse(value.strip_prefix('v').unwrap_or(value)).ok()?;
    let floor = semver::Version::parse(
        version::MIN_COMPOZY_VERSION
            .strip_prefix('v')
            .unwrap_or(version::MIN_COMPOZY_VERSION),
    )
    .expect("valid minimum CompozyOS version");
    if daemon >= floor {
        return None;
    }
    version::check(Some(value)).map(|warning| format!("version mismatch — {warning}"))
}

async fn wait_or_quit(events: &mut EventStream) -> bool {
    tokio::select! {
        biased;
        quit = wait_for_quit(events) => quit,
        _ = tokio::time::sleep(Duration::from_secs(3)) => false,
    }
}

async fn wait_for_quit(events: &mut EventStream) -> bool {
    while let Some(event) = events.next().await {
        if matches!(event, Ok(Event::Key(key)) if is_quit_key(key)) {
            return true;
        }
    }
    false
}

fn is_quit_key(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
        && (key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)))
}

fn retry_quit() -> AppError {
    AppError::reported(0)
}

fn last_probe_error(report: &ProbeReport) -> String {
    report
        .targets
        .iter()
        .find_map(|target| match &target.outcome {
            Outcome::Error(detail) => Some(format_probe_error(&target.transport, detail)),
            Outcome::Ok | Outcome::Skipped => None,
        })
        .unwrap_or_else(|| "probe did not return an error detail".into())
}

fn format_probe_error(transport: &Transport, detail: &str) -> String {
    match (transport, detail.strip_prefix("not found: ")) {
        (Transport::Uds(_), Some(path)) => format!("no socket file — uds {path}"),
        _ => format!(
            "{detail} — {} {}",
            transport_name(transport),
            target_name(transport)
        ),
    }
}

fn transport_name(transport: &Transport) -> &'static str {
    match transport {
        Transport::Uds(_) => "uds",
        Transport::Tcp(_) => "tcp",
    }
}

fn target_name(transport: &Transport) -> String {
    match transport {
        Transport::Uds(path) => path.display().to_string(),
        Transport::Tcp(addr) => addr.clone(),
    }
}

fn render_retry_screen(frame: &mut Frame<'_>, attempt: u32, last_error: &str) {
    frame.render_widget(
        Paragraph::new(format!(
            " batuta — connecting\n\n   connecting to daemon…  (attempt {attempt}, retrying every 3s)\n\n   last error: {last_error}\n\n   q  quit"
        )),
        frame.area(),
    );
}

#[cfg(unix)]
async fn termination_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut hangup = signal(SignalKind::hangup()).expect("install SIGHUP handler");
    tokio::select! { _ = terminate.recv() => {}, _ = hangup.recv() => {} }
}

pub const fn tty_required_error() -> &'static str {
    "batuta needs a terminal; use `batuta sessions --json` for scripting"
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    fn render(attempt: u32, error: &str) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        terminal
            .draw(|frame| render_retry_screen(frame, attempt, error))
            .unwrap();
        terminal.backend().to_string()
    }

    #[test]
    fn ut_017_retry_screen_shows_attempt_counter() {
        assert!(
            render(4, "connection refused — uds /tmp/compozy/daemon.sock").contains("attempt 4")
        );
    }

    #[test]
    fn ut_018_retry_screen_shows_specific_probe_error() {
        let report = ProbeReport {
            chosen: None,
            targets: vec![compozy_client::TargetOutcome {
                transport: Transport::Uds(PathBuf::from("/tmp/compozy/daemon.sock")),
                outcome: Outcome::Error("connection refused".into()),
            }],
        };
        assert!(render(1, &last_probe_error(&report)).contains("last error: connection refused"));
    }

    #[test]
    fn ut_019_quitting_retry_screen_is_a_clean_exit_before_a_client_returns() {
        let error = retry_quit();
        assert_eq!(error.exit_code(), 0);
        assert!(error.was_reported());
    }

    #[test]
    fn ut_020_startup_version_mismatch_retries_with_distinct_text() {
        let status = compozy_client::types::StatusPayload {
            daemon: compozy_client::types::DaemonStatus {
                version: Some("v0.2.0".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let mismatch = startup_version_mismatch(&status).unwrap();
        let connection = render(1, "connection refused — uds /tmp/compozy/daemon.sock");
        let version_mismatch = render(1, &mismatch);
        assert_ne!(connection, version_mismatch);
        assert!(version_mismatch.contains("version mismatch"));
    }

    #[test]
    fn startup_allows_supported_and_unverified_daemon_versions() {
        for daemon_version in ["v0.3.0-beta.16", "v0.4.0", "dev", "garbage"] {
            let status = compozy_client::types::StatusPayload {
                daemon: compozy_client::types::DaemonStatus {
                    version: Some(daemon_version.into()),
                    ..Default::default()
                },
                ..Default::default()
            };
            assert_eq!(startup_version_mismatch(&status), None, "{daemon_version}");
        }
    }

    #[test]
    fn retry_error_names_missing_uds_socket() {
        assert_eq!(
            format_probe_error(
                &Transport::Uds(PathBuf::from("/tmp/compozy/daemon.sock")),
                "not found: /tmp/compozy/daemon.sock",
            ),
            "no socket file — uds /tmp/compozy/daemon.sock"
        );
    }
}
