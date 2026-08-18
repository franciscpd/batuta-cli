use crate::{
    cli::Cli, config::Settings, doctor, exit::AppError, probe, terminal, version, workspace,
};
use batuta_tui::app::{AppMode, Model, Toast, ToastKind, WorkspaceRef};
use std::io::IsTerminal;

pub async fn run(cli: &Cli, settings: &Settings) -> Result<(), AppError> {
    let (report, client) = probe(cli).await;
    let Some(client) = client else {
        let report = doctor::Report {
            probe: report,
            status: None,
            workspace: None,
            warnings: vec!["daemon unreachable".into()],
            config: None,
        };
        eprint!("{}", doctor::render_human_error(&report));
        return Err(AppError::reported(1));
    };
    let status = client.status().await?;
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
    if !std::io::stdout().is_terminal() {
        return Err(AppError::daemon(tty_required_error()));
    }
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
    run_model(model, client).await
}

pub async fn run_model(model: Model, client: compozy_client::Client) -> Result<(), AppError> {
    terminal::install_panic_hook();
    let mut terminal = ratatui::init();
    let _guard = terminal::TerminalGuard::enter(terminal::RatatuiOps)
        .map_err(|error| AppError::daemon(format!("initialize terminal: {error}")))?;
    #[cfg(unix)]
    let result = tokio::select! {
        result = batuta_tui::runtime::run(model, client, &mut terminal) => result,
        _ = termination_signal() => Ok(()),
    };
    #[cfg(not(unix))]
    let result = batuta_tui::runtime::run(model, client, &mut terminal).await;
    result.map_err(AppError::daemon)
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
