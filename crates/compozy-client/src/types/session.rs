use super::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Page {
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub limit: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SessionPage {
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub page: Page,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Activity {
    #[serde(default)]
    pub last_activity_at: Option<Timestamp>,
    #[serde(default)]
    pub last_activity_kind: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Session {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub agent_name: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(rename = "type", default)]
    pub type_: Option<String>,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub badge: Option<String>,
    #[serde(default)]
    pub transcript_epoch: Option<i64>,
    #[serde(default)]
    pub activity: Option<Activity>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_detail: Option<String>,
    #[serde(default)]
    pub archived_at: Option<Timestamp>,
    #[serde(default)]
    pub created_at: Option<Timestamp>,
    #[serde(default)]
    pub updated_at: Option<Timestamp>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SessionResponse {
    #[serde(default)]
    pub session: Session,
}
