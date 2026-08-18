use super::Timestamp;
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LoopRun {
    pub id: String,
    #[serde(default)]
    pub workspace_id: String,
    pub loop_name: String,
    pub status: String,
    pub generation: i64,
    pub created_at: Timestamp,
    pub started_at: Timestamp,
    pub last_progress_at: Timestamp,
    #[serde(default)]
    pub parent_loop_run_id: String,
    #[serde(default)]
    pub inputs: Value,
    #[serde(default)]
    pub tokens_used: i64,
    #[serde(default)]
    pub pause_requested: bool,
    #[serde(default)]
    pub active_gate_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LoopRunPage {
    #[serde(default)]
    pub runs: Vec<LoopRun>,
    pub aggregates: LoopRunAggregates,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct LoopRunAggregates {
    pub total: u64,
    pub live: u64,
    pub terminal: u64,
    pub succeeded: u64,
    pub failed: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LoopRunDetail {
    pub run: LoopRun,
    #[serde(default)]
    pub materialized_contract: Value,
    #[serde(default)]
    pub generations: Vec<LoopGeneration>,
    #[serde(default)]
    pub node_controls: Vec<Value>,
    #[serde(default)]
    pub waits: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LoopGeneration {
    pub generation: i64,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub outputs: Vec<LoopGenerationOutput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LoopGenerationOutput {
    pub node_id: String,
    #[serde(default)]
    pub item_index: i64,
    pub status: String,
    #[serde(default)]
    pub attempt: i64,
    #[serde(default)]
    pub child_loop_run_id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub resolved_runtime: Option<Value>,
    #[serde(default)]
    pub failure_class: String,
    #[serde(default)]
    pub disposition: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LoopEvent {
    pub id: String,
    pub seq: i64,
    pub kind: String,
    #[serde(default)]
    pub payload: Value,
    pub at: Timestamp,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct LoopMutation {
    pub ok: bool,
    pub run_id: String,
    #[serde(default)]
    pub status: String,
}
