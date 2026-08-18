use crate::{
    Client, Error,
    request::RouteKind,
    sessions::encode_segment,
    types::{LoopMutation, LoopRunDetail, LoopRunPage},
};
use form_urlencoded::Serializer;
use serde::Serialize;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoopRunQuery<'a> {
    pub loop_: Option<&'a str>,
    pub status: Option<&'a str>,
    pub limit: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunControl {
    Pause,
    Resume,
    Cancel,
    Kill,
}

impl RunControl {
    fn wire(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Cancel => "cancel",
            Self::Kill => "kill",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    Approve,
    Reject,
}

impl Client {
    pub async fn loop_runs(
        &self,
        workspace: &str,
        query: &LoopRunQuery<'_>,
    ) -> Result<LoopRunPage, Error> {
        let mut serializer = Serializer::new(String::new());
        if let Some(loop_name) = query.loop_ {
            serializer.append_pair("loop", loop_name);
        }
        if let Some(status) = query.status {
            serializer.append_pair("status", status);
        }
        if query.limit > 0 {
            serializer.append_pair("limit", &query.limit.to_string());
        }
        let query = serializer.finish();
        let mut path = format!("/api/workspaces/{}/loop-runs", encode_segment(workspace));
        if !query.is_empty() {
            path.push('?');
            path.push_str(&query);
        }
        self.get_json(&path, "loop runs response", RouteKind::Scoped)
            .await
    }

    pub async fn loop_run(&self, workspace: &str, id: &str) -> Result<LoopRunDetail, Error> {
        self.get_json(
            &loop_run_path(workspace, id),
            "loop run response",
            RouteKind::Scoped,
        )
        .await
    }

    pub async fn loop_run_control(
        &self,
        workspace: &str,
        id: &str,
        control: RunControl,
    ) -> Result<LoopMutation, Error> {
        self.post_json(
            &format!("{}/{}", loop_run_path(workspace, id), control.wire()),
            &serde_json::json!({}),
            "loop run control response",
            RouteKind::Scoped,
        )
        .await
    }

    pub async fn loop_run_approve(
        &self,
        workspace: &str,
        id: &str,
        gate_id: &str,
        decision: GateDecision,
    ) -> Result<(), Error> {
        #[derive(Serialize)]
        struct Request<'a> {
            gate_id: &'a str,
            decision: GateDecision,
        }
        #[derive(serde::Deserialize)]
        struct Response {
            ok: bool,
        }
        let response: Response = self
            .post_json(
                &format!("{}/approve", loop_run_path(workspace, id)),
                &Request { gate_id, decision },
                "loop run approve response",
                RouteKind::Scoped,
            )
            .await?;
        if response.ok {
            Ok(())
        } else {
            Err(Error::UnexpectedPayload(200))
        }
    }
}

pub(crate) fn loop_run_path(workspace: &str, id: &str) -> String {
    format!(
        "/api/workspaces/{}/loop-runs/{}",
        encode_segment(workspace),
        encode_segment(id)
    )
}
