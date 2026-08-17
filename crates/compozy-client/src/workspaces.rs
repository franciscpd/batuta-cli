use crate::{Client, Error, request::RouteKind, types::WorkspacesResponse};

impl Client {
    pub async fn workspaces(&self) -> Result<Vec<crate::types::Workspace>, Error> {
        let response: WorkspacesResponse = self
            .get_json("/api/workspaces", "workspaces response", RouteKind::Fixed)
            .await?;
        Ok(response.workspaces)
    }
}
