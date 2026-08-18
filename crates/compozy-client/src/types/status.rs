use super::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct StatusPayload {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub generated_at: Option<Timestamp>,
    #[serde(default)]
    pub daemon: DaemonStatus,
    #[serde(default)]
    pub sessions: Value,
    #[serde(default)]
    pub subprocess_health: Value,
    #[serde(default)]
    pub health: Value,
    #[serde(default)]
    pub memory: Value,
    #[serde(default)]
    pub automation: Value,
    #[serde(default)]
    pub tasks: Value,
    #[serde(default)]
    pub bridges: Value,
    #[serde(default)]
    pub skills: Value,
    #[serde(default)]
    pub config: Value,
    #[serde(default)]
    pub log_tail: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DaemonStatus {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub socket: Option<String>,
    #[serde(default)]
    pub http_host: Option<String>,
    #[serde(default)]
    pub http_port: Option<u16>,
    #[serde(default)]
    pub started_at: Option<Timestamp>,
}
