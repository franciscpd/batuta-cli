use crate::{
    Client, Error,
    request::RouteKind,
    sessions::{encode_segment, session_base_path, session_path},
    types::{Clarification, ClarificationsResponse, ClarifyAnswer, ClarifyResult},
};

impl Client {
    pub async fn clarifications(
        &self,
        workspace: &str,
        session_id: &str,
    ) -> Result<Vec<Clarification>, Error> {
        let response: ClarificationsResponse = self
            .get_json(
                &session_path(workspace, session_id, "clarifications"),
                "clarifications response",
                RouteKind::Scoped,
            )
            .await?;
        Ok(response.clarifications)
    }

    pub async fn answer_clarification(
        &self,
        workspace: &str,
        session_id: &str,
        request_id: &str,
        answer: &ClarifyAnswer,
    ) -> Result<ClarifyResult, Error> {
        let path = format!(
            "{}/clarifications/{}/answer",
            session_base_path(workspace, session_id),
            encode_segment(request_id)
        );
        self.post_json(
            &path,
            answer,
            "clarification answer response",
            RouteKind::Scoped,
        )
        .await
    }
}
