use crate::{cli::Cli, exit::AppError, probe, version, workspace};
use compozy_client::{
    Outcome, ProbeReport, TargetOutcome, Transport,
    types::{StatusPayload, Workspace},
};
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct Report {
    pub probe: ProbeReport,
    pub status: Option<StatusPayload>,
    pub workspace: Option<Workspace>,
    pub warnings: Vec<String>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.status.is_some()
    }
}

pub async fn run(cli: &Cli) -> Result<(), AppError> {
    let (probe, client) = probe(cli).await;
    let Some(client) = client else {
        if let Some(message) = unexpected_probe_error(&probe) {
            return Err(AppError::daemon(message));
        }
        let report = Report {
            probe,
            status: None,
            workspace: None,
            warnings: vec!["daemon unreachable".into()],
        };
        if cli.json {
            print!("{}", render_json(&report));
        } else {
            eprint!("{}", render_human_error(&report));
        }
        return Err(AppError::reported(1));
    };
    let status = client.status().await?;
    let warnings = version::check(status.daemon.version.as_deref())
        .into_iter()
        .collect::<Vec<_>>();
    let explicit = cli.workspace.as_deref();
    let workspace = match workspace::resolve_from_daemon(&client, explicit).await {
        Ok(workspace) => Some(workspace),
        Err(error)
            if explicit.is_none() && error.to_string().starts_with("no workspace contains") =>
        {
            None
        }
        Err(error) => return Err(error),
    };
    let report = Report {
        probe,
        status: Some(status),
        workspace,
        warnings,
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
    if status.daemon.status == "draining" {
        output.push_str("note: writes are refused while draining; reads work\n");
    }
    output.push_str("ok\n");
    output
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
    warnings: &'a [String],
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
        warnings: &report.warnings,
        batuta: Batuta {
            version: env!("CARGO_PKG_VERSION"),
            min_compozy_version: version::MIN_COMPOZY_VERSION,
        },
    })
    .expect("doctor JSON serializes")
        + "\n"
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
    use std::path::PathBuf;
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
            warnings: vec![],
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
}
