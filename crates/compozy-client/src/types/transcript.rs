use super::Timestamp;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TranscriptPage {
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub epoch: i64,
    #[serde(default)]
    pub generation: i64,
    #[serde(default)]
    pub max_sequence: i64,
    #[serde(default)]
    pub has_older: bool,
    #[serde(default)]
    pub next_before_sequence: Option<i64>,
    #[serde(default)]
    pub limit: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Entry {
    #[serde(default)]
    pub message: UiMessage,
    #[serde(default)]
    pub start_sequence: i64,
    #[serde(default)]
    pub sequence: i64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UiMessage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub role: Role,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Unknown,
    System,
    User,
    Assistant,
    Other(String),
}

impl<'de> Deserialize<'de> for Role {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(deserializer)?.as_str() {
            "system" => Self::System,
            "user" => Self::User,
            "assistant" => Self::Assistant,
            value => Self::Other(value.to_owned()),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum Part {
    Text {
        text: String,
        state: Option<String>,
    },
    Reasoning {
        text: String,
        state: Option<String>,
    },
    Tool {
        name: String,
        tool_call_id: Option<String>,
        state: Option<String>,
        input: Option<Value>,
        output: Option<Value>,
        error_text: Option<String>,
        title: Option<String>,
    },
    File {
        filename: Option<String>,
        media_type: Option<String>,
        url: Option<String>,
    },
    Event {
        data: Value,
    },
    Marker {
        kind: Option<String>,
        summary: Option<String>,
        occurred_at: Option<Timestamp>,
        evidence: Option<Value>,
    },
    Permission {
        data: Value,
    },
    Unknown {
        type_: String,
    },
}

impl Default for Part {
    fn default() -> Self {
        Self::Unknown {
            type_: String::new(),
        }
    }
}

impl<'de> Deserialize<'de> for Part {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("part must be an object"))?;
        let type_ = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let string = |key: &str| object.get(key).and_then(Value::as_str).map(str::to_owned);
        let data = || object.get("data").cloned().unwrap_or(Value::Null);
        Ok(match type_ {
            "text" => Self::Text {
                text: string("text").unwrap_or_default(),
                state: string("state"),
            },
            "reasoning" => Self::Reasoning {
                text: string("text").unwrap_or_default(),
                state: string("state"),
            },
            "file" => Self::File {
                filename: string("filename"),
                media_type: string("mediaType"),
                url: string("url"),
            },
            "data-compozy-permission" => Self::Permission { data: data() },
            "data-compozy-event" => {
                let event = data();
                let event_type = event
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if event_type.starts_with("transcript_marker.") {
                    Self::Marker {
                        kind: event.get("kind").and_then(Value::as_str).map(str::to_owned),
                        summary: event
                            .get("summary")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        occurred_at: event
                            .get("occurred_at")
                            .cloned()
                            .and_then(|v| serde_json::from_value(v).ok()),
                        evidence: event.get("evidence").cloned(),
                    }
                } else {
                    Self::Event { data: event }
                }
            }
            "dynamic-tool" => Self::Tool {
                name: string("toolName").unwrap_or_default(),
                tool_call_id: string("toolCallId"),
                state: string("state"),
                input: object.get("input").cloned(),
                output: object.get("output").cloned(),
                error_text: string("errorText"),
                title: string("title"),
            },
            _ if type_.starts_with("tool-") => Self::Tool {
                name: type_.trim_start_matches("tool-").to_owned(),
                tool_call_id: string("toolCallId"),
                state: string("state"),
                input: object.get("input").cloned(),
                output: object.get("output").cloned(),
                error_text: string("errorText"),
                title: string("title"),
            },
            _ => Self::Unknown {
                type_: type_.to_owned(),
            },
        })
    }
}
