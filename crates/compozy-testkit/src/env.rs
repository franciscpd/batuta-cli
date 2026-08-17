use crate::{Error, Result};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) const SKIP_REASON: &str = "set COMPOZY_TEST_DAEMON_BIN";

pub(crate) fn discover(explicit: Option<&Path>, search_path: bool) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        return validate(path).map(Some);
    }
    if let Some(path) = env::var_os("COMPOZY_TEST_DAEMON_BIN") {
        return validate(Path::new(&path)).map(Some);
    }
    if !search_path {
        return Ok(None);
    }
    let Some(path) = env::var_os("PATH") else {
        return Ok(None);
    };
    for dir in env::split_paths(&path) {
        let candidate = dir.join("compozy");
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn validate(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        Ok(path.to_path_buf())
    } else {
        Err(Error::Message(format!(
            "COMPOZY_TEST_DAEMON_BIN does not name a file: {}",
            path.display()
        )))
    }
}

pub(crate) fn configure(command: &mut Command, home: &Path, path: Option<&std::ffi::OsStr>) {
    command.env_clear();
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command
        .env("COMPOZY_HOME", home)
        .env("HOME", home)
        .env("TZ", "UTC")
        .env("LANG", "C.UTF-8")
        .current_dir(home);
}
