use crate::{Client, Error, request::RouteKind, types::LogsResponse};
use form_urlencoded::Serializer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogQuery<'a> {
    pub workspace_id: &'a str,
    pub session_id: Option<&'a str>,
    pub run: Option<&'a str>,
    pub error_only: bool,
    pub after_seq: Option<i64>,
    pub limit: u64,
}

impl LogQuery<'_> {
    pub(crate) fn encode(&self) -> String {
        let mut serializer = Serializer::new(String::new());
        serializer.append_pair("workspace_id", self.workspace_id);
        if let Some(session_id) = self.session_id {
            serializer.append_pair("session_id", session_id);
        }
        if let Some(run) = self.run {
            serializer.append_pair("run", run);
        }
        if self.error_only {
            serializer.append_pair("error_only", "true");
        }
        if let Some(after_seq) = self.after_seq {
            serializer.append_pair("after_seq", &after_seq.to_string());
        }
        if self.limit > 0 {
            serializer.append_pair("limit", &self.limit.to_string());
        }
        serializer.finish()
    }
}

impl Client {
    pub async fn logs(&self, query: &LogQuery<'_>) -> Result<Vec<crate::types::LogEvent>, Error> {
        let response: LogsResponse = self
            .get_json(
                &format!("/api/logs?{}", query.encode()),
                "logs response",
                RouteKind::Scoped,
            )
            .await?;
        Ok(response.events)
    }
}
