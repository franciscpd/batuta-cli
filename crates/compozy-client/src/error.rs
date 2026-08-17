use serde_json::Value;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("transport: {0}")]
    Transport(String),
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    #[error(
        "{message}{suffix}",
        suffix = code.as_ref().map(|value| format!(" ({value})")).unwrap_or_default()
    )]
    Daemon {
        status: u16,
        message: String,
        code: Option<String>,
        details: Option<Value>,
    },
    #[error("route missing in this daemon version: {method} {path}")]
    RouteMissing { method: &'static str, path: String },
    #[error("daemon is draining")]
    Draining,
    #[error("decode {context}: {source}")]
    Decode {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("unexpected status payload (HTTP {0})")]
    UnexpectedPayload(u16),
}
