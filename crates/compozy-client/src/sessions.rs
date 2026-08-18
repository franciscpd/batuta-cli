use crate::{
    Client, Error,
    request::RouteKind,
    types::{Session, SessionPage, SessionResponse},
};
use form_urlencoded::Serializer;

#[derive(Clone, Debug)]
pub struct SessionQuery<'a> {
    pub workspace: &'a str,
    pub type_: &'a str,
    pub sort: &'a str,
    pub limit: u64,
    pub agent: Option<&'a str>,
}

impl Client {
    pub async fn sessions(&self, query: &SessionQuery<'_>) -> Result<SessionPage, Error> {
        let mut serializer = Serializer::new(String::new());
        serializer
            .append_pair("workspace", query.workspace)
            .append_pair("type", query.type_)
            .append_pair("sort", query.sort)
            .append_pair("limit", &query.limit.to_string());
        if let Some(agent) = query.agent {
            serializer.append_pair("agent", agent);
        }
        let path = format!("/api/sessions?{}", serializer.finish());
        self.get_json(&path, "sessions response", RouteKind::Scoped)
            .await
    }

    pub async fn session(&self, workspace: &str, id: &str) -> Result<Session, Error> {
        let path = format!(
            "/api/workspaces/{}/sessions/{}",
            encode_segment(workspace),
            encode_segment(id)
        );
        let response: SessionResponse = self
            .get_json(&path, "session response", RouteKind::Scoped)
            .await?;
        Ok(response.session)
    }
}

pub(crate) fn encode_segment(value: &str) -> String {
    form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
