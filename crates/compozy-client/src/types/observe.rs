use super::Timestamp;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Overview {
    pub attention: AttentionOverview,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AttentionOverview {
    pub total: u64,
    #[serde(default)]
    pub by_kind: BTreeMap<String, u64>,
    #[serde(default)]
    pub items: Vec<AttentionItem>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AttentionItem {
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub session_id: String,
    pub occurred_at: Timestamp,
    #[serde(default)]
    pub actions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct OverviewResponse {
    pub overview: Overview,
}
