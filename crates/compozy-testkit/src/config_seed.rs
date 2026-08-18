use crate::Result;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(crate) fn short_socket_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "batuta-ct-{}-{}.sock",
        std::process::id(),
        SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(crate) fn write(home: &Path, port: u16, socket: &Path) -> Result<()> {
    fs::create_dir_all(home.join("agents/batuta"))?;
    fs::write(
        home.join("config.toml"),
        format!(
            "[http]\nhost = \"127.0.0.1\"\nport = {port}\n\n[daemon]\nsocket = {:?}\n",
            socket.display().to_string()
        ),
    )?;
    fs::write(
        home.join("agents/batuta/AGENT.md"),
        "---\nname: batuta\nprovider: claude\n---\n\nYou are batuta.\n",
    )?;
    Ok(())
}

pub(crate) fn free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}
