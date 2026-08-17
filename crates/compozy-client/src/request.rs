use crate::{Client, Error, HttpClient, types::ErrorPayload};
use bytes::Bytes;
use http::{Method, Request, StatusCode, header::HOST};
use http_body_util::{BodyExt, Empty};
use serde::de::DeserializeOwned;
use std::{error::Error as StdError, time::Instant};

pub(crate) enum RouteKind {
    Fixed,
    Scoped,
}

pub(crate) struct RawResponse {
    pub status: StatusCode,
    pub body: Bytes,
}

impl Client {
    pub(crate) async fn get(&self, path: &str) -> Result<RawResponse, Error> {
        let uri: http::Uri = format!("{}{path}", self.base_uri)
            .parse()
            .map_err(|error| Error::Transport(format!("invalid URI: {error}")))?;
        let mut builder = Request::builder().method(Method::GET).uri(uri);
        if matches!(self.transport, crate::Transport::Uds(_)) {
            builder = builder.header(HOST, "localhost");
        }
        let request = builder
            .body(Empty::<Bytes>::new())
            .map_err(|error| Error::Transport(error.to_string()))?;
        let started = Instant::now();
        let operation = async {
            let response = match &self.control {
                HttpClient::Uds(client) => client.request(request).await,
                HttpClient::Tcp(client) => client.request(request).await,
            }
            .map_err(|error| Error::Transport(error_chain(&error)))?;
            let status = response.status();
            let body = response
                .into_body()
                .collect()
                .await
                .map_err(|error| Error::Transport(error_chain(&error)))?
                .to_bytes();
            Ok::<_, Error>(RawResponse { status, body })
        };
        let result = tokio::time::timeout(self.request_timeout, operation)
            .await
            .map_err(|_| Error::Timeout(self.request_timeout))?;
        let status = result
            .as_ref()
            .map_or(0, |response| response.status.as_u16());
        tracing::debug!(
            target: "http.request",
            route = path,
            status,
            elapsed_ms = started.elapsed().as_millis() as u64
        );
        result
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        context: &'static str,
        route_kind: RouteKind,
    ) -> Result<T, Error> {
        let response = self.get(path).await?;
        if !response.status.is_success() {
            return Err(response_error(
                response.status,
                &response.body,
                path,
                context,
                route_kind,
            ));
        }
        serde_json::from_slice(&response.body).map_err(|source| Error::Decode { context, source })
    }
}

pub(crate) fn response_error(
    status: StatusCode,
    body: &[u8],
    path: &str,
    context: &'static str,
    route_kind: RouteKind,
) -> Error {
    if status == StatusCode::NOT_FOUND && matches!(route_kind, RouteKind::Fixed) {
        return Error::RouteMissing {
            method: "GET",
            path: path.to_owned(),
        };
    }
    match serde_json::from_slice::<ErrorPayload>(body) {
        Ok(payload) => Error::Daemon {
            status: status.as_u16(),
            message: payload.error,
            code: payload.code,
            details: payload.details,
        },
        Err(source) => Error::Decode { context, source },
    }
}

fn error_chain(error: &(dyn StdError + 'static)) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        message.push_str(": ");
        message.push_str(&error.to_string());
        source = error.source();
    }
    message
}
