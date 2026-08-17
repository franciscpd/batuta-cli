use crate::{
    cli::Cli,
    exit::AppError,
    probe,
    terminal::{RatatuiOps, TerminalGuard},
    version, workspace,
};
use batuta_tui::app::{FooterState, Model, SessionHeader};
use compozy_client::{
    Error, SessionQuery,
    types::{Session, SessionPage, Workspace},
};
use std::io::IsTerminal;

pub async fn run(
    cli: &Cli,
    requested_session: Option<&str>,
    all_agents: bool,
) -> Result<(), AppError> {
    if let Some(id) = requested_session {
        validate_session_id(id)?;
    }
    let (report, client) = probe(cli).await;
    let Some(client) = client else {
        let report = crate::doctor::Report {
            probe: report,
            status: None,
            workspace: None,
            warnings: vec!["daemon unreachable".into()],
        };
        eprint!("{}", crate::doctor::render_human_error(&report));
        return Err(AppError::reported(1));
    };
    let status = client.status().await?;
    let warning = version::check(status.daemon.version.as_deref());
    let workspace = workspace::resolve_from_daemon(&client, cli.workspace.as_deref()).await?;
    let selected_id = match requested_session {
        Some(id) => id.to_owned(),
        None => {
            let query = selection_query(&workspace.id, all_agents);
            let page = client.sessions(&query).await?;
            let Some(session) = select_first(&page) else {
                eprintln!("{}", no_session_error(&workspace.name));
                return Err(AppError::reported(1));
            };
            session.id.clone()
        }
    };
    let session = match client.session(&workspace.id, &selected_id).await {
        Ok(session) => session,
        Err(Error::Daemon { status: 404, .. }) => {
            return Err(AppError::daemon(session_not_found_error(
                &workspace.name,
                &selected_id,
            )));
        }
        Err(error) => return Err(error.into()),
    };
    if !std::io::stdout().is_terminal() {
        return Err(AppError::daemon(tty_required_error()));
    }
    run_terminal(client, workspace, session, warning).await
}

async fn run_terminal(
    client: compozy_client::Client,
    workspace: Workspace,
    session: Session,
    warning: Option<String>,
) -> Result<(), AppError> {
    let mut model = Model::new(SessionHeader {
        workspace: workspace.name,
        workspace_id: workspace.id,
        session_id: session.id,
        agent: session.agent_name,
        name: session.name,
        state: session.state.clone(),
        warning,
    });
    if session.state == "stopped" {
        model.footer = FooterState::Stopped {
            reason: session.stop_reason,
            detail: session.stop_detail,
        };
    }
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard::enter(RatatuiOps)
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

fn selection_query(workspace: &str, all_agents: bool) -> SessionQuery<'_> {
    SessionQuery {
        workspace,
        type_: "user",
        sort: "recent",
        limit: 1,
        agent: (!all_agents).then_some("batuta"),
    }
}

fn select_first(page: &SessionPage) -> Option<&Session> {
    page.sessions.first()
}

fn no_session_error(workspace: &str) -> String {
    format!(
        "error: no batuta session in workspace {workspace}\nhint: run `batuta sessions --all-agents` or pass --session <id>"
    )
}

fn session_not_found_error(workspace: &str, session_id: &str) -> String {
    format!("session not found in workspace {workspace}: {session_id}")
}

fn tty_required_error() -> &'static str {
    "tail needs a terminal; use `batuta sessions --json` for scripting"
}

fn validate_session_id(id: &str) -> Result<(), AppError> {
    let suffix = id
        .strip_prefix("sess-")
        .or_else(|| id.strip_prefix("sess_"));
    if suffix.is_some_and(|value| {
        value.len() >= 16
            && value
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    }) {
        Ok(())
    } else {
        Err(AppError::usage(
            "--session needs the full id (sess-…); run `batuta sessions`",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compozy_client::types::Page;

    #[test]
    fn ut_100_default_query_is_batuta_user_recent_limit_one() {
        let query = selection_query("ws", false);
        assert_eq!(query.workspace, "ws");
        assert_eq!(query.agent, Some("batuta"));
        assert_eq!(query.type_, "user");
        assert_eq!(query.sort, "recent");
        assert_eq!(query.limit, 1);
    }
    #[test]
    fn ut_101_all_agents_drops_filter_and_keeps_daemon_order() {
        assert_eq!(selection_query("ws", true).agent, None);
        let page = SessionPage {
            sessions: vec![
                Session {
                    id: "first".into(),
                    ..Session::default()
                },
                Session {
                    id: "second".into(),
                    ..Session::default()
                },
            ],
            page: Page::default(),
        };
        assert_eq!(select_first(&page).unwrap().id, "first");
    }
    #[test]
    fn ut_102_empty_selection_has_exact_error_and_hint() {
        let error = no_session_error("batuta-cli");
        assert!(error.contains("no batuta session in workspace batuta-cli"));
        assert!(error.contains("batuta sessions --all-agents"));
    }
    #[test]
    fn ut_103_session_not_found_has_exact_error() {
        let error = session_not_found_error("batuta-cli", "sess-0000000000000000");
        assert_eq!(
            error,
            "session not found in workspace batuta-cli: sess-0000000000000000"
        );
    }
    #[test]
    fn ut_104_full_session_id_validation() {
        assert!(validate_session_id("sess-0000000000000000").is_ok());
        assert!(validate_session_id("sess_6574c31447dcf803f5b435334c483b02").is_ok());
        assert_eq!(validate_session_id("807cee97").unwrap_err().exit_code(), 2);
    }
    #[test]
    fn ut_105_non_tty_refusal_has_exact_message() {
        assert_eq!(
            tty_required_error(),
            "tail needs a terminal; use `batuta sessions --json` for scripting"
        );
    }
}
