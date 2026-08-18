use batuta_tui::{
    cmd::{Request, StreamId},
    msg::{AnyStreamEvent, ApiResponse, ApiResult},
    runtime::RuntimeClient,
};
use futures_util::{future::BoxFuture, stream::BoxStream};
use std::{
    collections::{HashMap, VecDeque},
    future::pending,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

#[derive(Clone, Default)]
pub struct FakeRuntimeClient {
    inner: Arc<Mutex<State>>,
}
#[derive(Default)]
struct State {
    requests: Vec<Request>,
    responses: VecDeque<ApiResult>,
    streams: HashMap<StreamId, VecDeque<AnyStreamEvent>>,
    pending: bool,
}
impl FakeRuntimeClient {
    pub fn push_response(&self, response: ApiResult) {
        self.inner.lock().unwrap().responses.push_back(response);
    }
    pub fn script_stream(&self, id: StreamId, events: Vec<AnyStreamEvent>) {
        self.inner.lock().unwrap().streams.insert(id, events.into());
    }
    pub fn requests(&self) -> Vec<Request> {
        self.inner.lock().unwrap().requests.clone()
    }
    pub fn set_pending(&self, pending: bool) {
        self.inner.lock().unwrap().pending = pending;
    }
}
impl RuntimeClient for FakeRuntimeClient {
    fn get(&self, request: Request) -> BoxFuture<'static, ApiResult> {
        self.execute(request)
    }
    fn post(&self, request: Request) -> BoxFuture<'static, ApiResult> {
        self.execute(request)
    }
    fn stream(
        &self,
        id: StreamId,
        _cursor: watch::Receiver<String>,
    ) -> BoxStream<'static, AnyStreamEvent> {
        let events = self
            .inner
            .lock()
            .unwrap()
            .streams
            .remove(&id)
            .unwrap_or_default();
        Box::pin(futures_util::stream::iter(events))
    }
}
impl FakeRuntimeClient {
    fn execute(&self, request: Request) -> BoxFuture<'static, ApiResult> {
        let mut state = self.inner.lock().unwrap();
        state.requests.push(request);
        if state.pending {
            return Box::pin(pending());
        }
        let response = state
            .responses
            .pop_front()
            .unwrap_or(Ok(ApiResponse::Empty));
        Box::pin(async move { response })
    }
}
