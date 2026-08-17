use crate::{Client, Error};
use http::Uri;
use hyper_util::rt::TokioIo;
use std::{
    future::Future,
    io,
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::net::UnixStream;
use tower_service::Service;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transport {
    Uds(PathBuf),
    Tcp(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportOrder {
    Auto,
    UdsOnly,
    TcpOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeReport {
    pub chosen: Option<Transport>,
    pub targets: Vec<TargetOutcome>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetOutcome {
    pub transport: Transport,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Error(String),
    Skipped,
}

#[derive(Clone, Debug)]
pub(crate) struct UnixConnector {
    path: PathBuf,
}

impl UnixConnector {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Service<Uri> for UnixConnector {
    type Response = TokioIo<UnixStream>;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _uri: Uri) -> Self::Future {
        let path = self.path.clone();
        Box::pin(async move { UnixStream::connect(path).await.map(TokioIo::new) })
    }
}

impl Client {
    pub async fn probe(
        order: TransportOrder,
        uds_path: PathBuf,
        tcp_addr: &str,
        timeout: Duration,
    ) -> (ProbeReport, Option<Self>) {
        let uds_transport = Transport::Uds(uds_path.clone());
        let tcp_transport = Transport::Tcp(tcp_addr.to_owned());
        let mut targets = Vec::with_capacity(2);

        if order == TransportOrder::TcpOnly {
            targets.push(TargetOutcome {
                transport: uds_transport,
                outcome: Outcome::Skipped,
            });
        } else {
            let started = Instant::now();
            let result = match std::fs::metadata(&uds_path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    Err(format!("not found: {}", uds_path.display()))
                }
                _ => probe_client(Self::uds(uds_path.clone()), timeout).await,
            };
            let outcome = outcome(&result);
            tracing::debug!(
                target: "transport.probe",
                kind = "uds",
                target = %uds_path.display(),
                outcome = outcome_label(&outcome),
                elapsed_ms = started.elapsed().as_millis() as u64
            );
            targets.push(TargetOutcome {
                transport: uds_transport.clone(),
                outcome,
            });
            if result.is_ok() {
                targets.push(TargetOutcome {
                    transport: tcp_transport,
                    outcome: Outcome::Skipped,
                });
                return (
                    ProbeReport {
                        chosen: Some(uds_transport),
                        targets,
                    },
                    result.ok(),
                );
            }
        }

        if order == TransportOrder::UdsOnly {
            targets.push(TargetOutcome {
                transport: tcp_transport,
                outcome: Outcome::Skipped,
            });
            return (
                ProbeReport {
                    chosen: None,
                    targets,
                },
                None,
            );
        }

        let started = Instant::now();
        let result = probe_client(Self::tcp(tcp_addr), timeout).await;
        let outcome = outcome(&result);
        tracing::debug!(
            target: "transport.probe",
            kind = "tcp",
            target = tcp_addr,
            outcome = outcome_label(&outcome),
            elapsed_ms = started.elapsed().as_millis() as u64
        );
        targets.push(TargetOutcome {
            transport: tcp_transport.clone(),
            outcome,
        });
        let chosen = result.as_ref().ok().map(|_| tcp_transport);
        (ProbeReport { chosen, targets }, result.ok())
    }
}

async fn probe_client(client: Client, timeout: Duration) -> Result<Client, String> {
    match tokio::time::timeout(timeout, client.status()).await {
        Err(_) => Err("timeout".to_owned()),
        Ok(Ok(_)) => Ok(client),
        Ok(Err(error)) => Err(probe_reason(&error)),
    }
}

fn probe_reason(error: &Error) -> String {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("connection refused") {
        "connection refused".to_owned()
    } else if matches!(error, Error::Timeout(_)) {
        "timeout".to_owned()
    } else {
        error.to_string()
    }
}

fn outcome(result: &Result<Client, String>) -> Outcome {
    match result {
        Ok(_) => Outcome::Ok,
        Err(reason) => Outcome::Error(reason.clone()),
    }
}

fn outcome_label(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Ok => "ok",
        Outcome::Error(_) => "error",
        Outcome::Skipped => "skipped",
    }
}
