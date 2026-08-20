use crate::{
    Client, Error,
    request::{RouteKind, response_error},
    types::{AddWorkspaceOutcome, AddWorkspaceRequest, Workspace, WorkspacesResponse},
};
use http::StatusCode;

impl Client {
    pub async fn workspaces(&self) -> Result<Vec<crate::types::Workspace>, Error> {
        let response: WorkspacesResponse = self
            .get_json("/api/workspaces", "workspaces response", RouteKind::Fixed)
            .await?;
        Ok(response.workspaces)
    }

    pub async fn add_workspace(
        &self,
        request: &AddWorkspaceRequest,
    ) -> Result<AddWorkspaceOutcome, Error> {
        let response = self.post_response("/api/workspaces", request).await?;
        let response = self.collect_response(response).await?;
        if matches!(
            response.status,
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ) {
            return Ok(AddWorkspaceOutcome::Unsupported);
        }
        if !response.status.is_success() {
            let error = response_error(
                response.status,
                &response.body,
                "POST",
                "/api/workspaces",
                "add workspace response",
                RouteKind::Fixed,
            );
            if matches!(
                &error,
                Error::Daemon {
                    code: Some(code),
                    ..
                } if code == "workspace.registration.unsupported"
            ) {
                return Ok(AddWorkspaceOutcome::Unsupported);
            }
            return Err(error);
        }
        let workspace = serde_json::from_slice::<Workspace>(&response.body).map_err(|source| {
            Error::Decode {
                context: "add workspace response",
                source,
            }
        })?;
        Ok(AddWorkspaceOutcome::Added(workspace))
    }
}
