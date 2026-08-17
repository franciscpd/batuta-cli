mod config_seed;
mod daemon;
mod env;
mod proxy;
mod readiness;

pub use daemon::{Daemon, DaemonOptions, StartOutcome};
pub use proxy::RequestLog;

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
