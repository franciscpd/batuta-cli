use super::Part;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ApproveRequest {
    pub request_id: String,
    pub turn_id: String,
    pub decision: Decision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Decision {
    #[serde(rename = "allow-once")]
    AllowOnce,
    #[serde(rename = "allow-always")]
    AllowAlways,
    #[serde(rename = "reject-once")]
    RejectOnce,
    #[serde(rename = "reject-always")]
    RejectAlways,
}

#[derive(Deserialize)]
pub(crate) struct ApprovalResponse {
    pub status: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PermissionData {
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub turn_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub raw: PermissionRaw,
}

impl PermissionData {
    pub fn from_part(part: &Part) -> Option<serde_json::Result<Self>> {
        match part {
            Part::Permission { data } => Some(serde_json::from_value(data.clone())),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PermissionRaw {
    #[serde(default)]
    pub tool_input: Value,
    #[serde(default)]
    pub options: Vec<PermissionOption>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionOption {
    #[serde(default)]
    pub decision: String,
    #[serde(default)]
    pub option_id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub label: String,
}
