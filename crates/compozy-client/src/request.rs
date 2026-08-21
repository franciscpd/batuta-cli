use crate::{Client, Error, HttpClient, RequestBody, types::ErrorPayload};
use bytes::Bytes;
use http::{
    Method, Request, StatusCode,
    header::{CONTENT_TYPE, HOST},
};
use http_body_util::{BodyExt, Empty, Full};
use hyper::{Response, body::Incoming};
use serde::{Serialize, de::DeserializeOwned};
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
        let response = self
            .request(Method::GET, path, Empty::<Bytes>::new().boxed(), false)
            .await?;
        self.collect(response).await
    }

    pub(crate) async fn post_response<B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<Response<Incoming>, Error> {
        let body = serde_json::to_vec(body)
            .map_err(|error| Error::Transport(format!("encode POST body: {error}")))?;
        self.request(
            Method::POST,
            path,
            Full::new(Bytes::from(body)).boxed(),
            true,
        )
        .await
    }

    pub(crate) async fn post_json<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
        context: &'static str,
        route_kind: RouteKind,
    ) -> Result<T, Error> {
        let response = self.post_response(path, body).await?;
        let response = self.collect(response).await?;
        if !response.status.is_success() {
            return Err(response_error(
                response.status,
                &response.body,
                "POST",
                path,
                context,
                route_kind,
            ));
        }
        serde_json::from_slice(&response.body).map_err(|source| Error::Decode { context, source })
    }

    pub(crate) async fn post_empty(
        &self,
        path: &str,
        context: &'static str,
        route_kind: RouteKind,
    ) -> Result<(), Error> {
        let response = self
            .request(Method::POST, path, Empty::<Bytes>::new().boxed(), false)
            .await?;
        if response.status().is_success() {
            return Ok(());
        }
        let response = self.collect(response).await?;
        Err(response_error(
            response.status,
            &response.body,
            "POST",
            path,
            context,
            route_kind,
        ))
    }

    pub(crate) async fn collect_response(
        &self,
        response: Response<Incoming>,
    ) -> Result<RawResponse, Error> {
        self.collect(response).await
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: RequestBody,
        json: bool,
    ) -> Result<Response<Incoming>, Error> {
        let uri: http::Uri = format!("{}{path}", self.base_uri)
            .parse()
            .map_err(|error| Error::Transport(format!("invalid URI: {error}")))?;
        let mut builder = Request::builder().method(method.clone()).uri(uri);
        if matches!(self.transport, crate::Transport::Uds(_)) {
            builder = builder.header(HOST, "localhost");
        }
        if json {
            builder = builder.header(CONTENT_TYPE, "application/json");
        }
        let request = builder
            .body(body)
            .map_err(|error| Error::Transport(error.to_string()))?;
        let started = Instant::now();
        let operation = async {
            match &self.control {
                HttpClient::Uds(client) => client.request(request).await,
                HttpClient::Tcp(client) => client.request(request).await,
            }
            .map_err(|error| Error::Transport(error_chain(&error)))
        };
        let result = tokio::time::timeout(self.request_timeout, operation)
            .await
            .map_err(|_| Error::Timeout(self.request_timeout))?;
        let status = result
            .as_ref()
            .map_or(0, |response| response.status().as_u16());
        tracing::debug!(
            target: "http.request",
            method = %method,
            route = path,
            status,
            elapsed_ms = started.elapsed().as_millis() as u64
        );
        result
    }

    async fn collect(&self, response: Response<Incoming>) -> Result<RawResponse, Error> {
        let status = response.status();
        let body = tokio::time::timeout(self.request_timeout, response.into_body().collect())
            .await
            .map_err(|_| Error::Timeout(self.request_timeout))?
            .map_err(|error| Error::Transport(error_chain(&error)))?
            .to_bytes();
        Ok(RawResponse { status, body })
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
                "GET",
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
    method: &'static str,
    path: &str,
    context: &'static str,
    route_kind: RouteKind,
) -> Error {
    if status == StatusCode::NOT_FOUND && matches!(route_kind, RouteKind::Fixed) {
        return Error::RouteMissing {
            method,
            path: path.to_owned(),
        };
    }
    match serde_json::from_slice::<ErrorPayload>(body) {
        Ok(payload) => Error::Daemon {
            status: status.as_u16(),
            message: payload.error,
            code: payload.code,
            details: payload.details,
            diagnostic: payload.diagnostic,
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
