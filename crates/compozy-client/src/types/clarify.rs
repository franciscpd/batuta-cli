use super::Timestamp;
use serde::{Deserialize, Serialize, ser::SerializeMap};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Clarification {
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub agent_name: String,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub asked_at: Option<Timestamp>,
    #[serde(default)]
    pub deadline: Option<Timestamp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClarifyAnswer {
    Choice(usize),
    Text(String),
}

impl Serialize for ClarifyAnswer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::Choice(choice) => map.serialize_entry("choice_index", choice)?,
            Self::Text(text) => map.serialize_entry("text", text)?,
        }
        map.end()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClarifyResult {
    #[serde(default)]
    pub choice: Option<usize>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub fallback: bool,
}

#[derive(Deserialize)]
pub(crate) struct ClarificationsResponse {
    #[serde(default)]
    pub clarifications: Vec<Clarification>,
}
