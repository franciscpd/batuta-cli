use crate::{Client, Error, request::RouteKind, types::OverviewResponse};

impl Client {
    pub async fn overview(&self, workspace: &str) -> Result<crate::types::Overview, Error> {
        let query: String = form_urlencoded::Serializer::new(String::new())
            .append_pair("workspace", workspace)
            .finish();
        let response: OverviewResponse = self
            .get_json(
                &format!("/api/observe/overview?{query}"),
                "observe overview response",
                RouteKind::Scoped,
            )
            .await?;
        Ok(response.overview)
    }
}
