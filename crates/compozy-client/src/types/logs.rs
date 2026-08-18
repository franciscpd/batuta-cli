use super::Timestamp;
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LogEvent {
    pub id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub workspace_id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub agent_name: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub content: Option<Value>,
    #[serde(default)]
    pub summary: String,
    pub timestamp: Timestamp,
}

#[derive(Deserialize)]
pub(crate) struct LogsResponse {
    pub events: Vec<LogEvent>,
}
