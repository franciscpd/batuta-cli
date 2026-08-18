use super::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PromptRequest {
    pub message: String,
    pub message_id: String,
    pub idempotency_key: String,
    pub mode: PromptMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<PromptRuntime>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptMode {
    #[default]
    Queue,
    Steer,
    Interrupt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PromptRuntime {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "the public contract requires Accepted(PromptResult) without indirection"
)]
pub enum PromptOutcome {
    Sent,
    Accepted(PromptResult),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct PromptResult {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub mode: PromptMode,
    #[serde(default)]
    pub delivery: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub idempotency_key: String,
    #[serde(default)]
    pub replayed: bool,
    #[serde(default)]
    pub queue_entry_id: Option<String>,
    #[serde(default)]
    pub queue_position: Option<u64>,
    #[serde(default)]
    pub queue_generation: Option<i64>,
    #[serde(default)]
    pub estimated_send_at: Option<Timestamp>,
    #[serde(default)]
    pub previous_turn_id: Option<String>,
    #[serde(default)]
    pub new_turn_id: Option<String>,
    #[serde(default)]
    pub canceled_queued_entries: Option<u64>,
}

#[derive(Deserialize)]
pub(crate) struct PromptResponse {
    pub prompt: PromptResult,
}
