pub mod error;
mod request;
mod sessions;
pub mod sse;
mod status;
mod transcript;
pub mod transport;
pub mod types;
mod workspaces;

pub use error::Error;
pub use sessions::SessionQuery;
pub use transcript::TranscriptQuery;
pub use transport::{Outcome, ProbeReport, TargetOutcome, Transport, TransportOrder};

use bytes::Bytes;
use http_body_util::Empty;
use hyper_util::client::legacy::{Client as HyperClient, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use std::{path::PathBuf, time::Duration};
use transport::UnixConnector;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

type UdsClient = HyperClient<UnixConnector, Empty<Bytes>>;
type TcpClient = HyperClient<HttpConnector, Empty<Bytes>>;

#[derive(Clone)]
pub(crate) enum HttpClient {
    Uds(UdsClient),
    Tcp(TcpClient),
}

#[derive(Clone)]
pub struct Client {
    pub(crate) transport: Transport,
    pub(crate) control: HttpClient,
    pub(crate) _sse: HttpClient,
    pub(crate) base_uri: String,
    pub(crate) request_timeout: Duration,
}

impl Client {
    pub fn uds(path: PathBuf) -> Self {
        let connector = UnixConnector::new(path.clone());
        let control = HyperClient::builder(TokioExecutor::new()).build(connector.clone());
        let sse = HyperClient::builder(TokioExecutor::new()).build(connector);
        Self {
            transport: Transport::Uds(path),
            control: HttpClient::Uds(control),
            _sse: HttpClient::Uds(sse),
            base_uri: "http://localhost".to_owned(),
            request_timeout: REQUEST_TIMEOUT,
        }
    }

    pub fn tcp(addr: impl Into<String>) -> Self {
        let addr = addr.into();
        let mut control_connector = HttpConnector::new();
        control_connector.enforce_http(true);
        let mut sse_connector = HttpConnector::new();
        sse_connector.enforce_http(true);
        Self {
            transport: Transport::Tcp(addr.clone()),
            control: HttpClient::Tcp(
                HyperClient::builder(TokioExecutor::new()).build(control_connector),
            ),
            _sse: HttpClient::Tcp(HyperClient::builder(TokioExecutor::new()).build(sse_connector)),
            base_uri: format!("http://{addr}"),
            request_timeout: REQUEST_TIMEOUT,
        }
    }

    pub fn transport(&self) -> &Transport {
        &self.transport
    }
}
