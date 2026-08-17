use crate::{Client, Error, request::RouteKind, sessions::encode_segment, types::TranscriptPage};
use form_urlencoded::Serializer;

#[derive(Clone, Copy, Debug)]
pub struct TranscriptQuery {
    pub limit: u64,
    pub before_sequence: Option<i64>,
}

impl Client {
    pub async fn transcript(
        &self,
        workspace: &str,
        id: &str,
        query: &TranscriptQuery,
    ) -> Result<TranscriptPage, Error> {
        let mut serializer = Serializer::new(String::new());
        serializer.append_pair("limit", &query.limit.to_string());
        if let Some(sequence) = query.before_sequence {
            serializer.append_pair("before_sequence", &sequence.to_string());
        }
        let path = format!(
            "/api/workspaces/{}/sessions/{}/transcript?{}",
            encode_segment(workspace),
            encode_segment(id),
            serializer.finish()
        );
        self.get_json(&path, "transcript response", RouteKind::Scoped)
            .await
    }
}
