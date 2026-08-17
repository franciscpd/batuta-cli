use compozy_client::{Client, types::Workspace};
use std::path::{Path, PathBuf};

use crate::exit::AppError;

pub async fn resolve_from_daemon(
    client: &Client,
    requested: Option<&str>,
) -> Result<Workspace, AppError> {
    let workspaces = client.workspaces().await?;
    resolve(
        &workspaces,
        requested,
        std::env::current_dir().map_err(|error| AppError::daemon(error.to_string()))?,
    )
}

pub fn resolve(
    workspaces: &[Workspace],
    requested: Option<&str>,
    cwd: PathBuf,
) -> Result<Workspace, AppError> {
    if let Some(value) = requested.filter(|value| !value.is_empty()) {
        return resolve_explicit(workspaces, value);
    }
    let cwd = std::fs::canonicalize(cwd).map_err(|error| AppError::daemon(error.to_string()))?;
    let matched = workspaces
        .iter()
        .filter_map(|workspace| {
            std::fs::canonicalize(&workspace.root_dir)
                .ok()
                .filter(|root| cwd.starts_with(root))
                .map(|root| (root.components().count(), workspace))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, workspace)| workspace.clone());
    matched.ok_or_else(|| no_workspace(&cwd))
}

pub fn resolve_explicit(workspaces: &[Workspace], value: &str) -> Result<Workspace, AppError> {
    if value.starts_with("ws_") || value.starts_with("ws-") {
        return workspaces
            .iter()
            .find(|workspace| workspace.id == value)
            .cloned()
            .ok_or_else(|| AppError::daemon(format!("workspace not found: {value}")));
    }
    if Path::new(value).is_absolute() {
        let candidate = std::fs::canonicalize(value)
            .map_err(|_| AppError::daemon(format!("workspace not found: {value}")))?;
        return workspaces
            .iter()
            .find(|workspace| {
                std::fs::canonicalize(&workspace.root_dir).ok().as_ref() == Some(&candidate)
            })
            .cloned()
            .ok_or_else(|| AppError::daemon(format!("workspace not found: {value}")));
    }
    let matches = workspaces
        .iter()
        .filter(|workspace| workspace.name == value)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(AppError::daemon(format!("workspace not found: {value}"))),
        [workspace] => Ok((*workspace).clone()),
        _ => Err(AppError::daemon(format!(
            "ambiguous workspace name: {value}; use the id"
        ))),
    }
}

pub fn no_workspace(cwd: &Path) -> AppError {
    AppError::daemon(format!(
        "no workspace contains {}; pass --workspace or set COMPOZY_WORKSPACE",
        cwd.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn workspace(id: &str, name: &str, root: &Path) -> Workspace {
        Workspace {
            id: id.into(),
            name: name.into(),
            root_dir: root.display().to_string(),
            ..Workspace::default()
        }
    }

    #[test]
    fn ut_080_explicit_id_name_and_path() {
        let temp = tempdir().unwrap();
        let ws = workspace("ws_1", "batuta-cli", temp.path());
        assert_eq!(
            resolve_explicit(std::slice::from_ref(&ws), "ws_1")
                .unwrap()
                .id,
            "ws_1"
        );
        assert_eq!(
            resolve_explicit(std::slice::from_ref(&ws), "batuta-cli")
                .unwrap()
                .id,
            "ws_1"
        );
        assert_eq!(
            resolve_explicit(&[ws], temp.path().to_str().unwrap())
                .unwrap()
                .id,
            "ws_1"
        );
    }
    #[test]
    fn ut_081_env_selection_is_an_explicit_selection() {
        let temp = tempdir().unwrap();
        let ws = workspace("ws_1", "batuta-cli", temp.path());
        assert_eq!(resolve_explicit(&[ws], "batuta-cli").unwrap().id, "ws_1");
    }
    #[test]
    fn ut_082_cwd_matches_workspace_root() {
        let temp = tempdir().unwrap();
        let nested = temp.path().join("crates/batuta");
        std::fs::create_dir_all(&nested).unwrap();
        let ws = workspace("ws_1", "batuta-cli", temp.path());
        assert_eq!(resolve(&[ws], None, nested).unwrap().id, "ws_1");
    }
    #[test]
    fn ut_083_unknown_flag_errors() {
        assert_eq!(
            resolve_explicit(&[], "nope").unwrap_err().to_string(),
            "workspace not found: nope"
        );
    }
    #[test]
    fn ut_084_longest_root_wins() {
        let temp = tempdir().unwrap();
        let inner = temp.path().join("a/b/c");
        std::fs::create_dir_all(&inner).unwrap();
        let a = workspace("ws_a", "a", &temp.path().join("a"));
        let b = workspace("ws_b", "b", &temp.path().join("a/b"));
        assert_eq!(resolve(&[a, b], None, inner).unwrap().id, "ws_b");
    }
    #[cfg(unix)]
    #[test]
    fn ut_085_symlinked_cwd_is_canonicalized() {
        use std::os::unix::fs::symlink;
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        let under = root.join("under");
        std::fs::create_dir_all(&under).unwrap();
        let link = temp.path().join("link");
        symlink(&under, &link).unwrap();
        assert_eq!(
            resolve(&[workspace("ws_1", "root", &root)], None, link)
                .unwrap()
                .id,
            "ws_1"
        );
    }
    #[test]
    fn ut_086_empty_catalog_errors() {
        let temp = tempdir().unwrap();
        assert!(
            resolve(&[], None, temp.path().into())
                .unwrap_err()
                .to_string()
                .starts_with("no workspace contains")
        );
    }
    #[test]
    fn ut_087_duplicate_name_is_ambiguous() {
        let temp = tempdir().unwrap();
        let ws = workspace("ws_1", "dup", temp.path());
        let mut other = ws.clone();
        other.id = "ws_2".into();
        assert_eq!(
            resolve_explicit(&[ws, other], "dup")
                .unwrap_err()
                .to_string(),
            "ambiguous workspace name: dup; use the id"
        );
    }
    #[test]
    fn ut_088_deleted_cwd_is_an_error() {
        assert!(resolve(&[], None, PathBuf::from("/definitely/not/a/directory")).is_err());
    }
}
