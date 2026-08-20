use crate::{cli::Cli, config::Settings, exit::AppError, probe, version, workspace};
use compozy_client::{
    Error, Outcome, ProbeReport, TargetOutcome, Transport,
    types::{StatusPayload, Workspace},
};
use serde::Serialize;
use std::time::{Duration, Instant};

const CATALOG_STREAM_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamCheck {
    Live { handshake_ms: u64 },
    Fatal { status: u16, cause: String },
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DaemonState {
    Connected,
    Draining,
    Offline,
}

impl DaemonState {
    fn derive(status: &StatusPayload, offline: bool) -> Self {
        if status.daemon.status == "draining" {
            Self::Draining
        } else if offline {
            Self::Offline
        } else {
            Self::Connected
        }
    }
}

#[derive(Clone, Debug)]
pub struct Report {
    pub probe: ProbeReport,
    pub status: Option<StatusPayload>,
    pub workspace: Option<Workspace>,
    pub streams: Option<StreamCheck>,
    pub warnings: Vec<String>,
    pub config: Option<ConfigReport>,
}

#[derive(Clone, Debug)]
pub struct ConfigReport {
    pub path: String,
    pub loaded: bool,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.status.is_some()
    }
}

pub async fn run(cli: &Cli, settings: &Settings) -> Result<(), AppError> {
    let (probe, client) = probe(cli).await;
    let Some(client) = client else {
        if let Some(message) = unexpected_probe_error(&probe) {
            return Err(AppError::daemon(message));
        }
        let report = Report {
            probe,
            status: None,
            workspace: None,
            streams: None,
            warnings: vec!["daemon unreachable".into()],
            config: Some(config_report(settings)),
        };
        if cli.json {
            print!("{}", render_json(&report));
        } else {
            eprint!("{}", render_human_error(&report));
        }
        return Err(AppError::reported(1));
    };
    let status = client.status().await?;
    let mut warnings = version::check(status.daemon.version.as_deref())
        .into_iter()
        .collect::<Vec<_>>();
    warnings.extend(settings.warnings.clone());
    let workspace = match workspace::resolve_from_daemon_with_source(
        &client,
        settings.workspace.as_deref(),
        settings.workspace_source,
    )
    .await?
    {
        workspace::WorkspaceResolution::Selected(workspace) => Some(workspace),
        workspace::WorkspaceResolution::Unresolved(_) => None,
    };
    let report = Report {
        probe,
        status: Some(status),
        workspace,
        streams: Some(probe_catalog_stream(&client).await),
        warnings,
        config: Some(config_report(settings)),
    };
    if cli.json {
        print!("{}", render_json(&report));
    } else {
        print!("{}", render_human(&report));
        for warning in &report.warnings {
            eprintln!("warning: {warning}");
        }
    }
    Ok(())
}

fn unexpected_probe_error(probe: &ProbeReport) -> Option<String> {
    probe
        .targets
        .iter()
        .find_map(|target| match &target.outcome {
            Outcome::Error(message) if message.starts_with("unexpected status payload") => {
                Some(message.clone())
            }
            _ => None,
        })
}

pub fn render_human(report: &Report) -> String {
    let mut output = transport_line(&report.probe, false);
    let status = report.status.as_ref().expect("successful doctor report");
    let version = status.daemon.version.as_deref().unwrap_or("unknown");
    output.push_str(&format!(
        "daemon      {}  {version}  schema {}\n",
        status.daemon.status, status.schema_version
    ));
    match &report.workspace {
        Some(workspace) => output.push_str(&format!(
            "workspace   {}  {}  {}\n",
            workspace.name, workspace.id, workspace.root_dir
        )),
        None => {
            let cwd = std::env::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| "current directory".into());
            output.push_str(&format!("workspace   none — no workspace contains {cwd}; pass --workspace or set COMPOZY_WORKSPACE\n"));
        }
    }
    if DaemonState::derive(status, false) == DaemonState::Draining {
        output.push_str("note: writes are refused while draining; reads work\n");
    }
    if let Some(stream) = &report.streams {
        output.push_str(&format!("streams     {}\n", render_stream_human(stream)));
    }
    if let Some(config) = &report.config {
        let state = if config.loaded {
            "loaded"
        } else {
            "not found — defaults"
        };
        output.push_str(&format!("config      {}  ({state})\n", config.path));
    }
    output.push_str("ok\n");
    output
}

async fn probe_catalog_stream(client: &compozy_client::Client) -> StreamCheck {
    let started = Instant::now();
    match tokio::time::timeout(CATALOG_STREAM_TIMEOUT, client.catalog_stream_handshake()).await {
        Ok(Ok(())) => StreamCheck::Live {
            handshake_ms: started.elapsed().as_millis() as u64,
        },
        Ok(Err(Error::Daemon {
            status, message, ..
        })) => StreamCheck::Fatal {
            status,
            cause: message,
        },
        Ok(Err(error)) => StreamCheck::Fatal {
            status: 0,
            cause: error.to_string(),
        },
        Err(_) => StreamCheck::Timeout,
    }
}

fn render_stream_human(stream: &StreamCheck) -> String {
    match stream {
        StreamCheck::Live { handshake_ms } => format!("catalog: live (handshake {handshake_ms}ms)"),
        StreamCheck::Fatal { status, cause } => format!("catalog: fatal ({status} — {cause})"),
        StreamCheck::Timeout => "catalog: timeout (after 2s)".into(),
    }
}

pub fn render_human_error(report: &Report) -> String {
    let mut output = transport_line(&report.probe, true);
    output.push_str("error: daemon unreachable\nstart it with: compozy start\n");
    output
}

fn transport_line(probe: &ProbeReport, include_errors: bool) -> String {
    let chosen = probe.chosen.as_ref().map(transport_name).unwrap_or("none");
    let mut output = String::new();
    if let Some(transport) = probe.chosen.as_ref() {
        output.push_str(&format!(
            "transport   {chosen}  {}\n",
            target_name(transport)
        ));
    } else {
        output.push_str("transport   none\n");
    }
    for target in &probe.targets {
        if let Outcome::Error(detail) = &target.outcome
            && (include_errors
                || probe
                    .chosen
                    .as_ref()
                    .is_some_and(|chosen| chosen != &target.transport))
        {
            output.push_str(&format!(
                "            {}  {}: {detail}\n",
                transport_name(&target.transport),
                target_name(&target.transport)
            ));
        }
    }
    output
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

#[derive(Serialize)]
struct JsonReport<'a> {
    ok: bool,
    transport: Option<&'static str>,
    targets: Vec<JsonTarget>,
    daemon: Option<JsonDaemon<'a>>,
    workspace: Option<JsonWorkspace<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    streams: Option<JsonStreams>,
    warnings: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<JsonConfig<'a>>,
    batuta: Batuta,
}
#[derive(Serialize)]
struct JsonTarget {
    kind: &'static str,
    target: String,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}
#[derive(Serialize)]
struct JsonDaemon<'a> {
    status: &'a str,
    version: &'a str,
    schema_version: &'a str,
}
#[derive(Serialize)]
struct JsonWorkspace<'a> {
    id: &'a str,
    name: &'a str,
    root_dir: &'a str,
}
#[derive(Serialize)]
struct Batuta {
    version: &'static str,
    min_compozy_version: &'static str,
}
#[derive(Serialize)]
struct JsonConfig<'a> {
    path: &'a str,
    loaded: bool,
}
#[derive(Serialize)]
struct JsonStreams {
    catalog: JsonStreamCheck,
}
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum JsonStreamCheck {
    Live { handshake_ms: u64 },
    Fatal { status: u16, cause: String },
    Timeout,
}

pub fn render_json(report: &Report) -> String {
    let targets = report
        .probe
        .targets
        .iter()
        .filter(|target| !matches!(target.outcome, Outcome::Skipped))
        .map(json_target)
        .collect();
    let daemon = report.status.as_ref().map(|status| JsonDaemon {
        status: &status.daemon.status,
        version: status.daemon.version.as_deref().unwrap_or("unknown"),
        schema_version: &status.schema_version,
    });
    let workspace = report.workspace.as_ref().map(|workspace| JsonWorkspace {
        id: &workspace.id,
        name: &workspace.name,
        root_dir: &workspace.root_dir,
    });
    serde_json::to_string(&JsonReport {
        ok: report.ok(),
        transport: report.probe.chosen.as_ref().map(transport_name),
        targets,
        daemon,
        workspace,
        streams: report.streams.as_ref().map(|stream| JsonStreams {
            catalog: json_stream_check(stream),
        }),
        warnings: &report.warnings,
        config: report.config.as_ref().map(|config| JsonConfig {
            path: &config.path,
            loaded: config.loaded,
        }),
        batuta: Batuta {
            version: env!("CARGO_PKG_VERSION"),
            min_compozy_version: version::MIN_COMPOZY_VERSION,
        },
    })
    .expect("doctor JSON serializes")
        + "\n"
}

fn json_stream_check(stream: &StreamCheck) -> JsonStreamCheck {
    match stream {
        StreamCheck::Live { handshake_ms } => JsonStreamCheck::Live {
            handshake_ms: *handshake_ms,
        },
        StreamCheck::Fatal { status, cause } => JsonStreamCheck::Fatal {
            status: *status,
            cause: cause.clone(),
        },
        StreamCheck::Timeout => JsonStreamCheck::Timeout,
    }
}

fn config_report(settings: &Settings) -> ConfigReport {
    ConfigReport {
        path: settings.config_path.display().to_string(),
        loaded: settings.loaded,
    }
}

fn json_target(target: &TargetOutcome) -> JsonTarget {
    let (outcome, detail) = match &target.outcome {
        Outcome::Ok => ("ok", None),
        Outcome::Error(detail) => ("error", Some(detail.clone())),
        Outcome::Skipped => ("skipped", Some("not tried".to_owned())),
    };
    JsonTarget {
        kind: transport_name(&target.transport),
        target: target_name(&target.transport),
        outcome,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, time::Duration};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };
    fn report(outcome: Outcome) -> Report {
        Report {
            probe: ProbeReport {
                chosen: if outcome == Outcome::Ok {
                    Some(Transport::Uds(PathBuf::from("/tmp/daemon.sock")))
                } else {
                    None
                },
                targets: vec![TargetOutcome {
                    transport: Transport::Uds(PathBuf::from("/tmp/daemon.sock")),
                    outcome,
                }],
            },
            status: Some(StatusPayload {
                schema_version: "x".into(),
                daemon: compozy_client::types::DaemonStatus {
                    status: "running".into(),
                    version: Some("v0.3.0-beta.16".into()),
                    ..Default::default()
                },
                ..Default::default()
            }),
            workspace: Some(Workspace {
                id: "ws_1".into(),
                name: "test".into(),
                root_dir: "/tmp".into(),
                ..Default::default()
            }),
            streams: Some(StreamCheck::Live { handshake_ms: 42 }),
            warnings: vec![],
            config: None,
        }
    }
    #[test]
    fn ut_060_human_uds_report() {
        let value = render_human(&report(Outcome::Ok));
        assert!(value.contains("transport   uds  /tmp/daemon.sock\n"));
        assert!(!value.contains("tcp"));
        assert!(value.ends_with("ok\n"));
    }
    #[test]
    fn ut_061_draining_note() {
        let mut value = report(Outcome::Ok);
        value.status.as_mut().unwrap().daemon.status = "draining".into();
        assert!(
            render_human(&value).contains("note: writes are refused while draining; reads work")
        );
    }
    #[test]
    fn ut_062_json_shape_is_one_line() {
        let mut value = report(Outcome::Ok);
        value.workspace = None;
        let rendered = render_json(&value);
        assert_eq!(rendered.matches('\n').count(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert!(parsed["workspace"].is_null());
        assert!(parsed["targets"][0].get("detail").is_none());
    }
    #[test]
    fn ut_063_unreachable_has_targets_and_hint() {
        let mut value = report(Outcome::Error("connection refused".into()));
        value.probe.targets.push(TargetOutcome {
            transport: Transport::Tcp("127.0.0.1:9999".into()),
            outcome: Outcome::Error("connection refused".into()),
        });
        assert!(render_human_error(&value).contains("tcp  127.0.0.1:9999: connection refused"));
    }

    #[test]
    fn ut_636_reports_config_in_human_and_json_output() {
        let mut value = report(Outcome::Ok);
        value.config = Some(ConfigReport {
            path: "/tmp/batuta/config.toml".into(),
            loaded: true,
        });
        assert!(render_human(&value).contains("config      /tmp/batuta/config.toml  (loaded)"));
        let json: serde_json::Value = serde_json::from_str(&render_json(&value)).unwrap();
        assert_eq!(json["config"]["path"], "/tmp/batuta/config.toml");
        assert_eq!(json["config"]["loaded"], true);
    }

    #[test]
    fn ut_014_human_streams_line_matches_contract() {
        assert!(
            render_human(&report(Outcome::Ok))
                .contains("streams     catalog: live (handshake 42ms)\n")
        );
    }

    #[test]
    fn ut_015_json_streams_shape_matches_contract() {
        let json: serde_json::Value =
            serde_json::from_str(&render_json(&report(Outcome::Ok))).unwrap();
        assert_eq!(
            json["streams"]["catalog"],
            serde_json::json!({
                "state": "live",
                "handshake_ms": 42,
            })
        );
    }

    #[tokio::test]
    async fn ut_011_catalog_probe_returns_live_after_a_healthy_handshake() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/sessions/catalog-stream"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let result =
            probe_catalog_stream(&compozy_client::Client::tcp(server.address().to_string())).await;
        assert!(matches!(result, StreamCheck::Live { handshake_ms } if handshake_ms < 2_000));
    }

    #[tokio::test]
    async fn ut_012_catalog_probe_reports_a_draining_503_as_fatal() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/sessions/catalog-stream"))
            .respond_with(
                ResponseTemplate::new(503)
                    .set_body_json(serde_json::json!({"error": "daemon draining"})),
            )
            .mount(&server)
            .await;

        assert_eq!(
            probe_catalog_stream(&compozy_client::Client::tcp(server.address().to_string())).await,
            StreamCheck::Fatal {
                status: 503,
                cause: "daemon draining".into(),
            }
        );
    }

    #[tokio::test]
    async fn ut_013_catalog_probe_times_out_at_the_two_second_boundary() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/sessions/catalog-stream"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(3)))
            .mount(&server)
            .await;

        let started = Instant::now();
        let result =
            probe_catalog_stream(&compozy_client::Client::tcp(server.address().to_string())).await;
        assert_eq!(result, StreamCheck::Timeout);
        assert!(started.elapsed() >= CATALOG_STREAM_TIMEOUT);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn ut_016_standalone_doctor_report_keeps_existing_checks_and_adds_streams() {
        let rendered = render_human(&report(Outcome::Ok));
        assert!(rendered.contains("transport   uds  /tmp/daemon.sock\n"));
        assert!(rendered.contains("daemon      running"));
        assert!(rendered.contains("workspace   test  ws_1  /tmp\n"));
        assert!(rendered.contains("streams     catalog: live (handshake 42ms)\n"));
        assert!(rendered.ends_with("ok\n"));
    }
}
