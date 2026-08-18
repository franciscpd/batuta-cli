use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ErrorPayload {
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub details: Option<Value>,
    #[serde(default)]
    pub diagnostic: Option<Value>,
}
