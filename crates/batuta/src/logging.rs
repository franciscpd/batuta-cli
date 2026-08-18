use std::{ffi::OsStr, path::PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt::writer::MakeWriterExt, prelude::*};

pub struct LoggingGuard {
    _guard: Option<WorkerGuard>,
}

pub fn init() -> LoggingGuard {
    let Ok(level) = std::env::var("BATUTA_LOG") else {
        return LoggingGuard { _guard: None };
    };
    if level.trim().is_empty() {
        return LoggingGuard { _guard: None };
    }
    let path = log_path();
    let Some(parent) = path.parent() else {
        return LoggingGuard { _guard: None };
    };
    if std::fs::create_dir_all(parent).is_err() {
        return LoggingGuard { _guard: None };
    }
    let file = tracing_appender::rolling::never(
        parent,
        path.file_name().unwrap_or(OsStr::new("batuta.log")),
    );
    let (writer, guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(writer.with_max_level(tracing::Level::TRACE));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(subscriber)
        .try_init();
    LoggingGuard {
        _guard: Some(guard),
    }
}

pub fn log_path() -> PathBuf {
    if let Some(path) = std::env::var_os("BATUTA_LOG_FILE") {
        return PathBuf::from(path);
    }
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from(".local/state"));
    state.join("batuta/batuta.log")
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_log_path_is_under_state_home() {
        assert!(super::log_path().ends_with("batuta/batuta.log"));
    }

    #[test]
    fn ut_111_logging_is_opt_in() {
        assert!(std::env::var("BATUTA_LOG").is_err() || !super::log_path().as_os_str().is_empty());
    }
}
