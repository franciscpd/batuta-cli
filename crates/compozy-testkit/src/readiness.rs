use compozy_client::Client;
use std::{
    fs,
    path::Path,
    process::Child,
    time::{Duration, Instant},
};

pub(crate) struct Failure {
    pub message: String,
    pub port_collision: bool,
}

pub(crate) async fn wait(
    child: &mut Child,
    socket: &Path,
    log: &Path,
    timeout: Duration,
) -> std::result::Result<(), Failure> {
    let started = Instant::now();
    loop {
        if tokio::time::timeout(
            Duration::from_millis(250),
            Client::uds(socket.to_path_buf()).status(),
        )
        .await
        .is_ok_and(|result| result.is_ok())
        {
            return Ok(());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(failure(
                    log,
                    format!("daemon exited before readiness ({status})"),
                ));
            }
            Ok(None) => {}
            Err(error) => return Err(failure(log, format!("poll daemon process: {error}"))),
        }
        if started.elapsed() >= timeout {
            return Err(failure(
                log,
                format!("daemon was not ready within {timeout:?}"),
            ));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn failure(log: &Path, reason: String) -> Failure {
    let contents = fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<_> = contents.lines().collect();
    let tail = lines[lines.len().saturating_sub(20)..].join("\n");
    let lower = contents.to_ascii_lowercase();
    Failure {
        message: format!("{reason}\nlast 20 lines of daemon-process.log:\n{tail}"),
        port_collision: lower.contains("address already in use"),
    }
}
