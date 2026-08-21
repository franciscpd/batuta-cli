use super::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Workspace {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub root_dir: String,
    #[serde(default)]
    pub add_dirs: Vec<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddWorkspaceRequest {
    pub name: String,
    pub root_dir: String,
}

#[derive(Clone, Debug)]
pub enum AddWorkspaceOutcome {
    Added(Workspace),
    Unsupported,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct WorkspacesResponse {
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
}
