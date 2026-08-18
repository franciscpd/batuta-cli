use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CatalogEvent {
    pub kind: String,
    pub workspace_id: String,
    pub session_id: String,
}
