use batuta_tui::{
    app::{Model, SessionHeader},
    msg::Msg,
    runtime::{RuntimeClient, run_with_messages},
};
use compozy_client::{StreamCursor, TranscriptEvent, types::TranscriptPage};
use futures_util::{future::BoxFuture, stream::BoxStream};
use ratatui::{Terminal, backend::TestBackend};
use std::{future::pending, time::Duration};
use tokio::sync::{mpsc, watch};

#[derive(Clone)]
struct NeverFetch;

impl RuntimeClient for NeverFetch {
    fn transcript_page(
        &self,
        _workspace: String,
        _session: String,
        _before: Option<i64>,
    ) -> BoxFuture<'static, Result<TranscriptPage, String>> {
        Box::pin(pending())
    }

    fn transcript_events(
        &self,
        _workspace: String,
        _session: String,
        _cursor: watch::Receiver<StreamCursor>,
    ) -> BoxStream<'static, TranscriptEvent> {
        Box::pin(futures_util::stream::pending())
    }
}

#[tokio::test]
async fn ut_192_quit_cancels_a_never_resolving_fetch() {
    let model = Model::new(SessionHeader {
        workspace: "workspace".into(),
        workspace_id: "ws-test".into(),
        session_id: "sess-test".into(),
        agent: "batuta".into(),
        name: None,
        state: "active".into(),
        warning: None,
    });
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
    let (sender, receiver) = mpsc::unbounded_channel();
    sender.send(Msg::Quit).expect("quit message");
    tokio::time::timeout(
        Duration::from_millis(100),
        run_with_messages(model, NeverFetch, &mut terminal, sender, receiver),
    )
    .await
    .expect("runtime exits promptly")
    .expect("runtime succeeds");
}
