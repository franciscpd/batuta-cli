mod support;
use batuta_tui::{
    app::{AppMode, Model, Settings},
    cmd::StreamId,
    msg::ApiResponse,
    msg::Msg,
    runtime::run_with_messages,
};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::test]
async fn ut_665_quit_aborts_all_tasks() {
    let model = Model::new(Settings::default(), AppMode::Full);
    let client = support::fake_client::FakeRuntimeClient::default();
    client.push_response(Ok(ApiResponse::Empty));
    client.script_stream(StreamId::Catalog, Vec::new());
    client.set_pending(true);
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let (sender, receiver) = mpsc::unbounded_channel();
    sender.send(Msg::Quit).unwrap();
    tokio::time::timeout(
        Duration::from_secs(1),
        run_with_messages(model, client, &mut terminal, sender, receiver),
    )
    .await
    .expect("runtime exits")
    .unwrap();
}

#[tokio::test]
async fn ut_662_initial_gets_execute_concurrently() {
    let model = Model::new(Settings::default(), AppMode::Full);
    let client = support::fake_client::FakeRuntimeClient::default();
    let inspect = client.clone();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let (sender, receiver) = mpsc::unbounded_channel();
    let quit = sender.clone();
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        let _ = quit.send(Msg::Quit);
    });
    run_with_messages(model, client, &mut terminal, sender, receiver)
        .await
        .unwrap();
    assert!(inspect.requests().len() >= 3);
}
