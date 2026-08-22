use crate::{
    app,
    cli::Cli,
    config::{Settings, resolve_color_depth},
    exit::AppError,
    probe, version, workspace,
};
use batuta_tui::{
    app::{ColorMode, FooterState, Model, Preset, SessionHeader, UiSettings},
    theme::Theme,
};
use compozy_client::{
    Error, SessionQuery,
    types::{Session, SessionPage, Workspace},
};
use std::io::IsTerminal;

pub async fn run(
    cli: &Cli,
    settings: &Settings,
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
            streams: None,
            warnings: vec!["daemon unreachable".into()],
            config: None,
        };
        eprint!("{}", crate::doctor::render_human_error(&report));
        return Err(AppError::reported(1));
    };
    let status = client.status().await?;
    let warning = version::check(status.daemon.version.as_deref());
    let workspace = match workspace::resolve_from_daemon_with_source(
        &client,
        settings.workspace.as_deref(),
        settings.workspace_source,
    )
    .await?
    {
        workspace::WorkspaceResolution::Selected(workspace) => workspace,
        workspace::WorkspaceResolution::Unresolved(candidate) => {
            return Err(workspace::no_workspace(&candidate.canonical_path));
        }
    };
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
    run_terminal(client, workspace, session, warning, settings).await
}

async fn run_terminal(
    client: compozy_client::Client,
    workspace: Workspace,
    session: Session,
    warning: Option<String>,
    settings: &Settings,
) -> Result<(), AppError> {
    let stopped = session.state == "stopped";
    let stop_reason = session.stop_reason.clone();
    let stop_detail = session.stop_detail.clone();
    let mut model = Model::tail(SessionHeader {
        workspace: workspace.name,
        workspace_id: workspace.id,
        session_id: session.id,
        agent: session.agent_name,
        name: session.name,
        state: session.state.clone(),
        warning,
    });
    if stopped && let Some(detail) = model.session_detail_mut() {
        detail.view.footer = FooterState::Stopped {
            reason: stop_reason,
            detail: stop_detail,
        };
    }
    apply_settings(&mut model, settings.preset.clone(), settings.ui.clone());
    app::run_model(model, client).await
}

fn apply_settings(model: &mut Model, preset: Preset, ui: UiSettings) {
    model.settings.preset = preset;
    model.settings.ui = ui;
    let depth = resolve_color_depth(
        model.settings.ui.color_depth,
        std::env::var("COLORTERM").ok().as_deref(),
    );
    model.theme = Theme::with_options(
        model.settings.ui.color != ColorMode::Never,
        model.settings.ui.theme.into(),
        std::env::var("COLORFGBG").ok().as_deref(),
        depth,
    );
    if let Some(detail) = model.session_detail_mut() {
        detail.view.render_cache.clear();
        detail.view.cache_dirty = true;
    }
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
    use batuta_tui::{app::ThemeMode, theme::ThemeVariant};
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

    #[test]
    fn ut_106_tail_applies_configured_theme_and_color_mode() {
        let mut model = Model::tail(SessionHeader::default());
        let mut ui = model.settings.ui.clone();
        ui.color = ColorMode::Never;
        ui.theme = ThemeMode::Light;
        let preset = model.settings.preset.clone();

        apply_settings(&mut model, preset, ui);

        assert!(!model.theme.color);
        assert_eq!(model.theme.variant, ThemeVariant::Light);
        assert!(model.session_detail().unwrap().view.cache_dirty);
    }
}
