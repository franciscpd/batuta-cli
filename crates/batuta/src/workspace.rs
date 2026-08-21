use compozy_client::{Client, types::Workspace};
use std::path::{Path, PathBuf};

use crate::exit::AppError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceSelectorSource {
    Flag,
    Environment,
}

impl WorkspaceSelectorSource {
    const fn label(self) -> &'static str {
        match self {
            Self::Flag => "--workspace",
            Self::Environment => "COMPOZY_WORKSPACE",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceCandidate {
    pub name: String,
    pub canonical_path: PathBuf,
}

#[derive(Clone, Debug)]
pub enum WorkspaceResolution {
    Selected(Workspace),
    Unresolved(WorkspaceCandidate),
}

pub async fn resolve_from_daemon_with_source(
    client: &Client,
    requested: Option<&str>,
    source: Option<WorkspaceSelectorSource>,
) -> Result<WorkspaceResolution, AppError> {
    let workspaces = client.workspaces().await?;
    let (flag, environment) = match source {
        Some(WorkspaceSelectorSource::Flag) => (requested, None),
        Some(WorkspaceSelectorSource::Environment) => (None, requested),
        None => (None, None),
    };
    resolve_startup(
        &workspaces,
        flag,
        environment,
        std::env::current_dir().map_err(|error| AppError::daemon(error.to_string()))?,
    )
}

pub fn resolve_startup(
    workspaces: &[Workspace],
    flag: Option<&str>,
    environment: Option<&str>,
    cwd: PathBuf,
) -> Result<WorkspaceResolution, AppError> {
    if let Some(value) = non_empty(flag) {
        return resolve_explicit_from(workspaces, value, WorkspaceSelectorSource::Flag)
            .map(WorkspaceResolution::Selected);
    }
    if let Some(value) = non_empty(environment) {
        return resolve_explicit_from(workspaces, value, WorkspaceSelectorSource::Environment)
            .map(WorkspaceResolution::Selected);
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
    Ok(match matched {
        Some(workspace) => WorkspaceResolution::Selected(workspace),
        None => WorkspaceResolution::Unresolved(candidate_from_canonical(cwd)),
    })
}

fn resolve_explicit_from(
    workspaces: &[Workspace],
    value: &str,
    source: WorkspaceSelectorSource,
) -> Result<Workspace, AppError> {
    if value.starts_with("ws_") || value.starts_with("ws-") {
        return workspaces
            .iter()
            .find(|workspace| workspace.id == value)
            .cloned()
            .ok_or_else(|| explicit_not_found(source, value));
    }
    if Path::new(value).is_absolute() {
        let candidate =
            std::fs::canonicalize(value).map_err(|_| explicit_not_found(source, value))?;
        return workspaces
            .iter()
            .find(|workspace| {
                std::fs::canonicalize(&workspace.root_dir).ok().as_ref() == Some(&candidate)
            })
            .cloned()
            .ok_or_else(|| explicit_not_found(source, value));
    }
    let matches = workspaces
        .iter()
        .filter(|workspace| workspace.name == value)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(explicit_not_found(source, value)),
        [workspace] => Ok((*workspace).clone()),
        _ => Err(AppError::daemon(format!(
            "ambiguous workspace name: {value}; use the id or absolute path (from {})",
            source.label()
        ))),
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

fn explicit_not_found(source: WorkspaceSelectorSource, value: &str) -> AppError {
    AppError::daemon(format!(
        "workspace from {} not found: {value}; use an id, unique name, or absolute path",
        source.label()
    ))
}

fn candidate_from_canonical(canonical_path: PathBuf) -> WorkspaceCandidate {
    let name = canonical_path
        .file_name()
        .and_then(|component| component.to_str())
        .filter(|component| !component.is_empty())
        .unwrap_or("workspace")
        .to_owned();
    WorkspaceCandidate {
        name,
        canonical_path,
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
    fn ut_725_precedence_prefers_flag_then_environment_then_cwd() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let catalog = [
            workspace("ws_flag", "flag", &temp.path().join("flag")),
            workspace("ws_env", "env", &temp.path().join("env")),
            workspace("ws_cwd", "cwd", &root),
        ];
        assert!(matches!(
            resolve_startup(&catalog, Some("ws_flag"), Some("ws_env"), nested.clone()),
            Ok(WorkspaceResolution::Selected(workspace)) if workspace.id == "ws_flag"
        ));
        assert!(matches!(
            resolve_startup(&catalog, None, Some("ws_env"), nested.clone()),
            Ok(WorkspaceResolution::Selected(workspace)) if workspace.id == "ws_env"
        ));
        assert!(matches!(
            resolve_startup(&catalog, Some(""), Some("ws_env"), nested.clone()),
            Ok(WorkspaceResolution::Selected(workspace)) if workspace.id == "ws_env"
        ));
        assert!(matches!(
            resolve_startup(&catalog, None, None, nested),
            Ok(WorkspaceResolution::Selected(workspace)) if workspace.id == "ws_cwd"
        ));
    }

    #[test]
    fn ut_726_explicit_errors_name_the_source_and_do_not_fall_through() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        let cwd = root.join("nested");
        std::fs::create_dir_all(&cwd).unwrap();
        let first = workspace("ws_1", "duplicate", &root);
        let mut second = first.clone();
        second.id = "ws_2".into();
        let error = resolve_startup(
            &[first.clone(), second],
            Some("duplicate"),
            None,
            cwd.clone(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("duplicate"));
        assert!(error.contains("--workspace"));
        let error = resolve_startup(&[first], None, Some("missing"), cwd)
            .unwrap_err()
            .to_string();
        assert!(error.contains("COMPOZY_WORKSPACE"));
        assert!(error.contains("missing"));
    }

    #[cfg(unix)]
    #[test]
    fn ut_727_canonical_paths_use_longest_component_prefix_and_symlinks() {
        use std::os::unix::fs::symlink;
        let temp = tempdir().unwrap();
        let root = temp.path().join("app");
        let nested_root = root.join("nested");
        let under = nested_root.join("under");
        std::fs::create_dir_all(&under).unwrap();
        let link = temp.path().join("link");
        symlink(&under, &link).unwrap();
        assert_eq!(
            match resolve_startup(
                &[
                    workspace("ws_root", "root", &root),
                    workspace("ws_nested", "nested", &nested_root),
                ],
                None,
                None,
                link,
            )
            .unwrap()
            {
                WorkspaceResolution::Selected(workspace) => workspace.id,
                WorkspaceResolution::Unresolved(_) => panic!("expected a selection"),
            },
            "ws_nested"
        );
        let application = temp.path().join("application");
        std::fs::create_dir_all(&application).unwrap();
        assert!(matches!(
            resolve_startup(
                &[workspace("ws_app", "app", &root)],
                None,
                None,
                application,
            ),
            Ok(WorkspaceResolution::Unresolved(_))
        ));
    }

    #[test]
    fn ut_728_unmatched_cwd_produces_a_canonical_candidate() {
        let temp = tempdir().unwrap();
        let directory = temp.path().join("new-project");
        std::fs::create_dir(&directory).unwrap();
        let expected = std::fs::canonicalize(&directory).unwrap();
        assert!(matches!(
            resolve_startup(&[], None, None, directory),
            Ok(WorkspaceResolution::Unresolved(WorkspaceCandidate { name, canonical_path }))
                if name == "new-project" && canonical_path == expected
        ));
        let root = if cfg!(windows) {
            PathBuf::from(r"C:\")
        } else {
            PathBuf::from("/")
        };
        assert_eq!(candidate_from_canonical(root).name, "workspace");
    }

    #[test]
    fn ut_730_client_and_views_keep_their_generic_and_pure_boundaries() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let client = root.join("crates/compozy-client");
        let manifest = std::fs::read_to_string(client.join("Cargo.toml")).unwrap();
        assert!(!manifest.to_ascii_lowercase().contains("ratatui"));
        for file in [
            client.join("src/workspaces.rs"),
            client.join("src/types/workspace.rs"),
        ] {
            assert!(
                !std::fs::read_to_string(file)
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains("batuta")
            );
        }
        let views = root.join("crates/batuta-tui/src/views");
        for entry in std::fs::read_dir(views).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                let source = std::fs::read_to_string(entry.path()).unwrap();
                assert!(!source.contains("compozy_client"));
                assert!(!source.contains("std::fs"));
            }
        }
    }

    #[test]
    fn e2e_704_resolution_journey_uses_flag_env_and_canonical_cwd() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        let nested = root.join("project");
        std::fs::create_dir_all(&nested).unwrap();
        let catalog = [
            workspace("ws_flag", "flag", &temp.path().join("flag")),
            workspace("ws_env", "env", &temp.path().join("env")),
            workspace("ws_cwd", "cwd", &root),
        ];
        for (flag, environment, expected) in [
            (Some("ws_flag"), Some("ws_env"), "ws_flag"),
            (None, Some("ws_env"), "ws_env"),
            (None, None, "ws_cwd"),
        ] {
            let WorkspaceResolution::Selected(workspace) =
                resolve_startup(&catalog, flag, environment, nested.clone()).unwrap()
            else {
                panic!("expected a selected workspace");
            };
            assert_eq!(workspace.id, expected);
        }
    }
}
