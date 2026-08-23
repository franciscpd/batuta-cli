use crate::{cli::Cli, config::Settings, exit::AppError, probe, version, workspace};
use compozy_client::{
    SessionQuery,
    types::{Session, SessionPage, Workspace},
};
use serde::Serialize;
use time::OffsetDateTime;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub async fn run(
    cli: &Cli,
    settings: &Settings,
    all_agents: bool,
    limit: i64,
) -> Result<(), AppError> {
    validate_limit(limit)?;
    let (report, client) = probe(cli).await;
    let Some(client) = client else {
        if !cli.json {
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
        }
        return Err(AppError::daemon("daemon unreachable"));
    };
    let status = client.status().await?;
    let warnings = version::check(status.daemon.version.as_deref())
        .into_iter()
        .collect::<Vec<_>>();
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
    let page = client
        .sessions(&SessionQuery {
            workspace: &workspace.id,
            type_: Some("user"),
            sort: "recent",
            limit: limit as u64,
            agent: (!all_agents).then_some("batuta"),
        })
        .await?;
    if cli.json {
        print!("{}", render_json(&workspace, &page, &warnings));
    } else {
        print!(
            "{}",
            render_table(&page, OffsetDateTime::now_utc(), terminal_width())
        );
        for warning in warnings {
            eprintln!("warning: {warning}");
        }
    }
    let _ = report;
    Ok(())
}

pub fn render_table(page: &SessionPage, now: OffsetDateTime, width: usize) -> String {
    if page.sessions.is_empty() {
        return "no sessions\n".to_owned();
    }
    let mut output =
        String::from("ID                       AGENT   STATE    LAST ACTIVITY  NAME\n");
    for session in &page.sessions {
        let id = &session.id;
        let agent = &session.agent_name;
        let state = &session.state;
        let activity = crate::relative_time::render(last_activity(session), now);
        let name = session
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or("—");
        let fixed = 25 + 2 + 8 + 2 + 8 + 2 + 15 + 2;
        let allowed = width.saturating_sub(fixed).max(1);
        output.push_str(&format!(
            "{id:<25}  {agent:<8}  {state:<8}  {activity:<15}  {}\n",
            truncate(name, allowed)
        ));
    }
    if page.page.has_more {
        output.push_str("more sessions available (raise --limit)\n");
    }
    output
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(120)
}
fn last_activity(session: &Session) -> Option<OffsetDateTime> {
    session
        .activity
        .as_ref()
        .and_then(|activity| activity.last_activity_at.as_ref())
        .or(session.updated_at.as_ref())
        .and_then(|timestamp| timestamp.parsed())
}

fn truncate(value: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(value) <= max_width {
        return value.to_owned();
    }
    if max_width == 1 {
        return "…".to_owned();
    }
    let mut width = 0;
    let mut output = String::new();
    for character in value.chars() {
        let character_width = character.width().unwrap_or(0);
        if width + character_width > max_width - 1 {
            break;
        }
        output.push(character);
        width += character_width;
    }
    output.push('…');
    output
}

#[derive(Serialize)]
struct JsonOutput<'a> {
    workspace: JsonWorkspace<'a>,
    warnings: &'a [String],
    sessions: Vec<JsonSession<'a>>,
    page: JsonPage,
}
#[derive(Serialize)]
struct JsonWorkspace<'a> {
    id: &'a str,
    name: &'a str,
}
#[derive(Serialize)]
struct JsonPage {
    has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<u64>,
    limit: u64,
}
#[derive(Serialize)]
struct JsonSession<'a> {
    id: &'a str,
    agent_name: &'a str,
    name: Option<&'a str>,
    state: &'a str,
    #[serde(rename = "type")]
    type_: Option<&'a str>,
    last_activity_at: &'a str,
    created_at: Option<&'a str>,
    updated_at: Option<&'a str>,
}

pub fn render_json(workspace: &Workspace, page: &SessionPage, warnings: &[String]) -> String {
    let sessions = page
        .sessions
        .iter()
        .map(|session| JsonSession {
            id: &session.id,
            agent_name: &session.agent_name,
            name: session.name.as_deref(),
            state: &session.state,
            type_: session.type_.as_deref(),
            last_activity_at: last_activity_raw(session),
            created_at: session.created_at.as_ref().map(|timestamp| timestamp.raw()),
            updated_at: session.updated_at.as_ref().map(|timestamp| timestamp.raw()),
        })
        .collect();
    serde_json::to_string(&JsonOutput {
        workspace: JsonWorkspace {
            id: &workspace.id,
            name: &workspace.name,
        },
        warnings,
        sessions,
        page: JsonPage {
            has_more: page.page.has_more,
            next_cursor: page.page.next_cursor.clone(),
            total: page.page.total,
            limit: page.page.limit,
        },
    })
    .expect("sessions JSON serializes")
        + "\n"
}

fn last_activity_raw(session: &Session) -> &str {
    session
        .activity
        .as_ref()
        .and_then(|activity| activity.last_activity_at.as_ref())
        .or(session.updated_at.as_ref())
        .map(|timestamp| timestamp.raw())
        .unwrap_or("")
}

fn validate_limit(limit: i64) -> Result<(), AppError> {
    if limit < 1 {
        Err(AppError::usage("--limit must be at least 1"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compozy_client::types::{Activity, Page, Timestamp};
    fn page() -> SessionPage {
        SessionPage {
            sessions: vec![
                Session {
                    id: "sess-1".into(),
                    agent_name: "batuta".into(),
                    state: "active".into(),
                    name: Some("A useful session".into()),
                    activity: Some(Activity {
                        last_activity_at: Some(Timestamp::from("2026-08-17T12:00:00Z".to_owned())),
                        ..Default::default()
                    }),
                    updated_at: Some(Timestamp::from("2026-08-16T12:00:00Z".to_owned())),
                    ..Default::default()
                },
                Session {
                    id: "sess-2".into(),
                    agent_name: "batuta".into(),
                    state: "stopped".into(),
                    name: None,
                    updated_at: Some(Timestamp::from("2026-08-16T12:00:12Z".to_owned())),
                    ..Default::default()
                },
            ],
            page: Page {
                limit: 20,
                ..Default::default()
            },
        }
    }
    #[test]
    fn ut_090_table_uses_activity_then_updated_at() {
        let now = OffsetDateTime::parse(
            "2026-08-17T12:00:12Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap();
        let output = render_table(&page(), now, 120);
        assert!(output.contains("12s ago"));
        assert!(output.contains("1d ago"));
    }
    #[test]
    fn ut_091_empty_name_states_and_truncation() {
        let mut value = page();
        value.sessions[0].state = "starting".into();
        value.sessions[0].name =
            Some("a deliberately very long session name that needs truncation".into());
        let output = render_table(&value, OffsetDateTime::now_utc(), 100);
        assert!(output.contains("starting"));
        assert!(output.contains('…'));
        assert!(output.contains("—"));
    }
    #[test]
    fn ut_092_limit_validation() {
        assert!(super::validate_limit(0).is_err());
        assert!(super::validate_limit(-1).is_err());
    }
    #[test]
    fn ut_093_more_footer() {
        let mut value = page();
        value.page.has_more = true;
        value.page.limit = 2;
        assert!(
            render_table(&value, OffsetDateTime::now_utc(), 120)
                .contains("more sessions available (raise --limit)")
        );
    }
    #[test]
    fn ut_094_json_shape_and_fallback() {
        let workspace = Workspace {
            id: "ws_1".into(),
            name: "test".into(),
            ..Default::default()
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&render_json(&workspace, &page(), &[])).unwrap();
        assert_eq!(parsed["workspace"]["id"], "ws_1");
        assert_eq!(
            parsed["sessions"][1]["last_activity_at"],
            "2026-08-16T12:00:12Z"
        );
    }
}
