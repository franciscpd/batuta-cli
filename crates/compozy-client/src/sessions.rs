use crate::{
    Client, Error,
    request::{RouteKind, response_error},
    types::{
        ApprovalResponse, ApproveRequest, PromptOutcome, PromptRequest, PromptResponse, Session,
        SessionPage, SessionResponse,
    },
};
use form_urlencoded::Serializer;
use http::{StatusCode, header::CONTENT_TYPE};
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct SessionQuery<'a> {
    pub workspace: &'a str,
    pub type_: Option<&'a str>,
    pub sort: &'a str,
    pub limit: u64,
    pub agent: Option<&'a str>,
}

impl Client {
    pub async fn create_session(&self, workspace: &str, agent: &str) -> Result<Session, Error> {
        #[derive(Serialize)]
        struct CreateSessionRequest<'a> {
            workspace: &'a str,
            agent_name: &'a str,
        }

        let response: SessionResponse = self
            .post_json(
                "/api/sessions",
                &CreateSessionRequest {
                    workspace,
                    agent_name: agent,
                },
                "create session response",
                RouteKind::Fixed,
            )
            .await?;
        Ok(response.session)
    }

    pub async fn sessions(&self, query: &SessionQuery<'_>) -> Result<SessionPage, Error> {
        let encoded = {
            let mut serializer = Serializer::new(String::new());
            serializer
                .append_pair("workspace", query.workspace)
                .append_pair("sort", query.sort)
                .append_pair("limit", &query.limit.to_string());
            if let Some(type_) = query.type_ {
                serializer.append_pair("type", type_);
            }
            if let Some(agent) = query.agent {
                serializer.append_pair("agent", agent);
            }
            serializer.finish()
        };
        let path = format!("/api/sessions?{encoded}");
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

    pub async fn send_prompt(
        &self,
        workspace: &str,
        id: &str,
        request: &PromptRequest,
    ) -> Result<PromptOutcome, Error> {
        let path = session_path(workspace, id, "prompt");
        let response = self.post_response(&path, request).await?;
        let status = response.status();
        let is_event_stream = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/event-stream"))
            });

        if !status.is_success() {
            let response = self.collect_response(response).await?;
            return Err(response_error(
                response.status,
                &response.body,
                "POST",
                &path,
                "prompt error response",
                RouteKind::Scoped,
            ));
        }
        if is_event_stream || status == StatusCode::OK {
            return Ok(PromptOutcome::Sent);
        }
        if status == StatusCode::ACCEPTED {
            let response = self.collect_response(response).await?;
            let payload: PromptResponse =
                serde_json::from_slice(&response.body).map_err(|source| Error::Decode {
                    context: "prompt response",
                    source,
                })?;
            return Ok(PromptOutcome::Accepted(payload.prompt));
        }
        Err(Error::UnexpectedPayload(status.as_u16()))
    }

    pub async fn cancel_prompt(&self, workspace: &str, id: &str) -> Result<(), Error> {
        self.post_empty(
            &session_path(workspace, id, "prompt/cancel"),
            "cancel prompt response",
            RouteKind::Scoped,
        )
        .await
    }

    pub async fn approve_permission(
        &self,
        workspace: &str,
        id: &str,
        request: &ApproveRequest,
    ) -> Result<(), Error> {
        let response: ApprovalResponse = self
            .post_json(
                &session_path(workspace, id, "approve"),
                request,
                "approve permission response",
                RouteKind::Scoped,
            )
            .await?;
        if response.status == "approved" {
            Ok(())
        } else {
            Err(Error::UnexpectedPayload(200))
        }
    }
}

pub(crate) fn session_path(workspace: &str, id: &str, suffix: &str) -> String {
    format!("{}/{}", session_base_path(workspace, id), suffix)
}

pub(crate) fn session_base_path(workspace: &str, id: &str) -> String {
    format!(
        "/api/workspaces/{}/sessions/{}",
        encode_segment(workspace),
        encode_segment(id)
    )
}

pub(crate) fn encode_segment(value: &str) -> String {
    form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
