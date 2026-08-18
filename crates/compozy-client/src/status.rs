use crate::{Client, Error, request::RouteKind, types::StatusPayload};
use http::StatusCode;

impl Client {
    pub async fn status(&self) -> Result<StatusPayload, Error> {
        const PATH: &str = "/api/status";
        let response = self.get(PATH).await?;
        if response.status.is_success() {
            return serde_json::from_slice(&response.body)
                .map_err(|_| Error::UnexpectedPayload(response.status.as_u16()));
        }
        if response.status == StatusCode::SERVICE_UNAVAILABLE
            && serde_json::from_slice::<crate::types::ErrorPayload>(&response.body)
                .is_ok_and(|payload| payload.error.contains("daemon is draining"))
        {
            let mut payload = StatusPayload::default();
            payload.daemon.status = "draining".to_owned();
            return Ok(payload);
        }
        let error = crate::request::response_error(
            response.status,
            &response.body,
            "GET",
            PATH,
            "status",
            RouteKind::Fixed,
        );
        match error {
            Error::Decode { .. } => Err(Error::UnexpectedPayload(response.status.as_u16())),
            error => Err(error),
        }
    }
}
