use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timestamp {
    raw: String,
    parsed: Option<OffsetDateTime>,
}

impl Timestamp {
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn parsed(&self) -> Option<OffsetDateTime> {
        self.parsed
    }
}

impl From<String> for Timestamp {
    fn from(raw: String) -> Self {
        let parsed = OffsetDateTime::parse(&raw, &Rfc3339).ok();
        Self { raw, parsed }
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(String::deserialize(deserializer)?.into())
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.raw)
    }
}
