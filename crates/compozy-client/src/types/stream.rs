use super::{Entry, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TranscriptSnapshot {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub epoch: i64,
    #[serde(default)]
    pub generation: i64,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub max_sequence: i64,
    #[serde(default)]
    pub has_older: bool,
    #[serde(default)]
    pub next_before_sequence: Option<i64>,
    #[serde(default)]
    pub reset: bool,
    #[serde(default)]
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TranscriptDelta {
    #[serde(default)]
    pub epoch: i64,
    #[serde(default)]
    pub generation: i64,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub cursor: i64,
    #[serde(default)]
    pub max_sequence: i64,
    #[serde(default)]
    pub has_more: bool,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SessionStopped {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_detail: Option<String>,
    #[serde(default)]
    pub failure: Option<Value>,
    #[serde(default)]
    pub timestamp: Option<Timestamp>,
}
