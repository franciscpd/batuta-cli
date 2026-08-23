use crate::{
    app::{Model, update},
    cmd::{Cmd, LogScope, Request, RequestId, StreamId, TimerId},
    msg::{AnyStreamEvent, ApiError, ApiResponse, ApiResult, Msg},
    views,
};
use compozy_client::{
    Client, LogCursor, LogQuery, LoopRunQuery, NoCursor, ReconnectPolicy, SeqCursor, SessionQuery,
    StreamCursor, TranscriptQuery,
};
use crossterm::event::{Event, EventStream};
use futures_util::{StreamExt, future::BoxFuture, stream::BoxStream};
use ratatui::{Terminal, backend::Backend};
use std::{collections::HashMap, fmt::Display, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

pub trait RuntimeClient: Clone + Send + Sync + 'static {
    fn get(&self, request: Request) -> BoxFuture<'static, ApiResult>;
    fn post(&self, request: Request) -> BoxFuture<'static, ApiResult>;
    fn stream(
        &self,
        id: StreamId,
        cursor: watch::Receiver<String>,
    ) -> BoxStream<'static, AnyStreamEvent>;
    fn stream_with_options(
        &self,
        id: StreamId,
        cursor: watch::Receiver<String>,
        _error_only: bool,
    ) -> BoxStream<'static, AnyStreamEvent> {
        self.stream(id, cursor)
    }
}

impl RuntimeClient for Client {
    fn get(&self, request: Request) -> BoxFuture<'static, ApiResult> {
        let client = self.clone();
        Box::pin(async move {
            match request {
                Request::Status { .. } => client
                    .status()
                    .await
                    .map(|value| ApiResponse::Status(Box::new(value))),
                Request::Workspaces { .. } => {
                    client.workspaces().await.map(ApiResponse::Workspaces)
                }
                Request::Sessions {
                    workspace,
                    agent,
                    limit,
                    ..
                } => client
                    .sessions(&SessionQuery {
                        workspace: &workspace,
                        type_: "user",
                        sort: "recent",
                        limit,
                        agent: agent.as_deref(),
                    })
                    .await
                    .map(|value| ApiResponse::Sessions(Box::new(value))),
                Request::Session {
                    workspace, session, ..
                } => client
                    .session(&workspace, &session)
                    .await
                    .map(ApiResponse::Session),
                Request::TranscriptPage {
                    workspace,
                    session,
                    before_sequence,
                    ..
                } => client
                    .transcript(
                        &workspace,
                        &session,
                        &TranscriptQuery {
                            limit: 200,
                            before_sequence,
                        },
                    )
                    .await
                    .map(|value| ApiResponse::TranscriptPage(Box::new(value))),
                Request::VisibleTranscript {
                    workspace, session, ..
                } => client
                    .transcript(
                        &workspace,
                        &session,
                        &TranscriptQuery {
                            limit: 200,
                            before_sequence: None,
                        },
                    )
                    .await
                    .map(|value| ApiResponse::TranscriptPage(Box::new(value))),
                Request::Runs {
                    workspace,
                    loop_name,
                    limit,
                    ..
                } => client
                    .loop_runs(
                        &workspace,
                        &LoopRunQuery {
                            loop_: loop_name.as_deref(),
                            status: None,
                            limit,
                        },
                    )
                    .await
                    .map(|value| ApiResponse::Runs(Box::new(value))),
                Request::Run { workspace, run, .. } => client
                    .loop_run(&workspace, &run)
                    .await
                    .map(|value| ApiResponse::Run(Box::new(value))),
                Request::Overview { workspace, .. } => client
                    .overview(&workspace)
                    .await
                    .map(|value| ApiResponse::Overview(Box::new(value))),
                Request::Logs {
                    workspace,
                    scope,
                    error_only,
                    limit,
                    ..
                } => {
                    let (session_id, run) = match &scope {
                        LogScope::Session(id) => (Some(id.as_str()), None),
                        LogScope::Run(id) => (None, Some(id.as_str())),
                        LogScope::Workspace => (None, None),
                    };
                    client
                        .logs(&LogQuery {
                            workspace_id: &workspace,
                            session_id,
                            run,
                            error_only,
                            after_seq: None,
                            limit,
                        })
                        .await
                        .map(ApiResponse::Logs)
                }
                Request::Clarifications {
                    workspace, session, ..
                } => client
                    .clarifications(&workspace, &session)
                    .await
                    .map(ApiResponse::Clarifications),
                _ => return Err("write request passed to get".into()),
            }
            .map_err(api_error_text)
        })
    }
    fn post(&self, request: Request) -> BoxFuture<'static, ApiResult> {
        let client = self.clone();
        Box::pin(async move {
            match request {
                Request::CreateSession {
                    workspace, agent, ..
                } => client
                    .create_session(&workspace, &agent)
                    .await
                    .map(ApiResponse::SessionCreated),
                Request::AddWorkspace { name, root_dir, .. } => client
                    .add_workspace(&compozy_client::types::AddWorkspaceRequest { name, root_dir })
                    .await
                    .map(ApiResponse::WorkspaceAdded),
                Request::Prompt {
                    workspace,
                    session,
                    prompt,
                    ..
                } => client
                    .send_prompt(&workspace, &session, &prompt)
                    .await
                    .map(ApiResponse::Prompt),
                Request::CancelPrompt {
                    workspace, session, ..
                } => client
                    .cancel_prompt(&workspace, &session)
                    .await
                    .map(|()| ApiResponse::Empty),
                Request::Approve {
                    workspace,
                    session,
                    request,
                    ..
                } => client
                    .approve_permission(&workspace, &session, &request)
                    .await
                    .map(|()| ApiResponse::Empty),
                Request::AnswerClarification {
                    workspace,
                    session,
                    request_id,
                    answer,
                    ..
                } => client
                    .answer_clarification(&workspace, &session, &request_id, &answer)
                    .await
                    .map(ApiResponse::ClarificationAnswered),
                Request::RunControl {
                    workspace,
                    run,
                    control,
                    ..
                } => client
                    .loop_run_control(&workspace, &run, control)
                    .await
                    .map(ApiResponse::RunMutation),
                Request::RunApprove {
                    workspace,
                    run,
                    gate_id,
                    decision,
                    ..
                } => client
                    .loop_run_approve(&workspace, &run, &gate_id, decision)
                    .await
                    .map(|()| ApiResponse::Empty),
                Request::TaskVerb {
                    workspace,
                    task_id,
                    verb,
                    ..
                } => client
                    .task_verb(&workspace, &task_id, verb)
                    .await
                    .map(|()| ApiResponse::Empty),
                _ => return Err("read request passed to post".into()),
            }
            .map_err(api_error_text)
        })
    }
    fn stream(
        &self,
        id: StreamId,
        cursor: watch::Receiver<String>,
    ) -> BoxStream<'static, AnyStreamEvent> {
        client_stream(self.clone(), String::new(), id, cursor, false)
    }
    fn stream_with_options(
        &self,
        id: StreamId,
        cursor: watch::Receiver<String>,
        error_only: bool,
    ) -> BoxStream<'static, AnyStreamEvent> {
        client_stream(self.clone(), String::new(), id, cursor, error_only)
    }
}

fn api_error_text(error: compozy_client::Error) -> ApiError {
    match error {
        compozy_client::Error::Daemon {
            status,
            message,
            code,
            details,
            diagnostic,
        } => ApiError {
            message,
            status: Some(status),
            code,
            details,
            diagnostic,
        },
        error => ApiError {
            message: error.to_string(),
            status: None,
            code: None,
            details: None,
            diagnostic: None,
        },
    }
}

#[derive(Clone)]
struct ScopedClient {
    client: Client,
    workspace: String,
}
impl RuntimeClient for ScopedClient {
    fn get(&self, request: Request) -> BoxFuture<'static, ApiResult> {
        self.client.get(request)
    }
    fn post(&self, request: Request) -> BoxFuture<'static, ApiResult> {
        self.client.post(request)
    }
    fn stream(
        &self,
        id: StreamId,
        cursor: watch::Receiver<String>,
    ) -> BoxStream<'static, AnyStreamEvent> {
        client_stream(
            self.client.clone(),
            self.workspace.clone(),
            id,
            cursor,
            false,
        )
    }
    fn stream_with_options(
        &self,
        id: StreamId,
        cursor: watch::Receiver<String>,
        error_only: bool,
    ) -> BoxStream<'static, AnyStreamEvent> {
        client_stream(
            self.client.clone(),
            self.workspace.clone(),
            id,
            cursor,
            error_only,
        )
    }
}

fn client_stream(
    client: Client,
    workspace: String,
    id: StreamId,
    mut cursor: watch::Receiver<String>,
    error_only: bool,
) -> BoxStream<'static, AnyStreamEvent> {
    match id {
        StreamId::Transcript(session) => {
            let initial = parse_transcript(&cursor.borrow().clone());
            let (tx, rx) = watch::channel(initial);
            tokio::spawn(async move {
                while cursor.changed().await.is_ok() {
                    if tx.send(parse_transcript(&cursor.borrow().clone())).is_err() {
                        break;
                    }
                }
            });
            Box::pin(
                client
                    .transcript_stream(&workspace, &session, rx, ReconnectPolicy::default())
                    .map(AnyStreamEvent::Transcript),
            )
        }
        StreamId::Catalog => {
            let (_tx, rx) = watch::channel(NoCursor);
            Box::pin(
                client
                    .catalog_stream(rx, ReconnectPolicy::default())
                    .map(AnyStreamEvent::Catalog),
            )
        }
        StreamId::RunEvents(run) => {
            let initial = cursor.borrow().parse().unwrap_or_default();
            let (tx, rx) = watch::channel(SeqCursor(initial));
            tokio::spawn(async move {
                while cursor.changed().await.is_ok() {
                    let value = cursor.borrow().parse().unwrap_or_default();
                    if tx.send(SeqCursor(value)).is_err() {
                        break;
                    }
                }
            });
            Box::pin(
                client
                    .loop_run_events(&workspace, &run, rx, ReconnectPolicy::default())
                    .map(AnyStreamEvent::Run),
            )
        }
        StreamId::Logs(scope) => {
            let initial = parse_log(&cursor.borrow().clone());
            let (tx, rx) = watch::channel(initial);
            tokio::spawn(async move {
                while cursor.changed().await.is_ok() {
                    if tx.send(parse_log(&cursor.borrow().clone())).is_err() {
                        break;
                    }
                }
            });
            let (session_id, run) = match &scope {
                LogScope::Session(id) => (Some(id.as_str()), None),
                LogScope::Run(id) => (None, Some(id.as_str())),
                LogScope::Workspace => (None, None),
            };
            let query = LogQuery {
                workspace_id: &workspace,
                session_id,
                run,
                error_only,
                after_seq: None,
                limit: 200,
            };
            Box::pin(
                client
                    .logs_stream(&query, rx, ReconnectPolicy::default())
                    .map(AnyStreamEvent::Logs),
            )
        }
    }
}

fn parse_transcript(value: &str) -> StreamCursor {
    let mut fields = value.split(':').filter_map(|part| part.parse().ok());
    StreamCursor {
        epoch: fields.next().unwrap_or_default(),
        generation: fields.next().unwrap_or_default(),
        after_sequence: fields.next().unwrap_or_default(),
    }
}
fn parse_log(value: &str) -> LogCursor {
    let Some((ts, seq)) = value.rsplit_once('|') else {
        return LogCursor::default();
    };
    LogCursor {
        ts: ts.into(),
        seq: seq.parse().unwrap_or_default(),
    }
}

struct StreamTask {
    task: JoinHandle<()>,
    cursor: watch::Sender<String>,
}
#[derive(Default)]
struct Tasks {
    requests: HashMap<RequestId, JoinHandle<()>>,
    streams: HashMap<StreamId, StreamTask>,
    timers: HashMap<TimerId, JoinHandle<()>>,
}
impl Tasks {
    async fn stop_stream(&mut self, id: &StreamId) {
        if let Some(task) = self.streams.remove(id) {
            task.task.abort();
            let _ = task.task.await;
        }
    }
    async fn stop_all(&mut self) {
        let mut handles = Vec::new();
        for (_, task) in self.requests.drain() {
            task.abort();
            handles.push(task);
        }
        for (_, task) in self.streams.drain() {
            task.task.abort();
            handles.push(task.task);
        }
        for (_, task) in self.timers.drain() {
            task.abort();
            handles.push(task);
        }
        for handle in handles {
            let _ = handle.await;
        }
    }
}

fn render_interval(fps: u16) -> Duration {
    Duration::from_secs_f64(1.0 / f64::from(fps.clamp(5, 60)))
}
pub async fn run<B>(model: Model, client: Client, terminal: &mut Terminal<B>) -> Result<(), String>
where
    B: Backend,
    B::Error: Display,
{
    let (sender, receiver) = mpsc::unbounded_channel();
    let event_sender = sender.clone();
    let event_task = tokio::spawn(async move {
        let mut events = EventStream::new();
        while let Some(event) = events.next().await {
            let message = match event {
                Ok(Event::Key(key)) => Msg::Key(key),
                Ok(Event::Mouse(mouse)) => Msg::Mouse(mouse),
                Ok(Event::Resize(width, height)) => Msg::Resize(width, height),
                Ok(Event::FocusGained) => Msg::TerminalFocus(true),
                Ok(Event::FocusLost) => Msg::TerminalFocus(false),
                Ok(_) => continue,
                Err(_) => break,
            };
            if event_sender.send(message).is_err() {
                break;
            }
        }
    });
    let tick_sender = sender.clone();
    let tick = render_interval(model.settings.ui.fps);
    let tick_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick);
        loop {
            interval.tick().await;
            if tick_sender.send(Msg::Tick).is_err() {
                break;
            }
        }
    });
    let scoped = ScopedClient {
        workspace: model
            .workspace
            .as_ref()
            .map(|value| value.id.clone())
            .unwrap_or_default(),
        client,
    };
    let result = run_with_messages(model, scoped, terminal, sender, receiver).await;
    event_task.abort();
    tick_task.abort();
    result
}

pub async fn run_with_messages<B, C>(
    mut model: Model,
    client: C,
    terminal: &mut Terminal<B>,
    sender: mpsc::UnboundedSender<Msg>,
    mut receiver: mpsc::UnboundedReceiver<Msg>,
) -> Result<(), String>
where
    B: Backend,
    B::Error: Display,
    C: RuntimeClient,
{
    let mut tasks = Tasks::default();
    let initial = model.initial_cmds();
    if execute(&model, &client, terminal, &sender, initial, &mut tasks).await? {
        return Ok(());
    }
    while let Some(message) = receiver.recv().await {
        match &message {
            Msg::Api { request, .. } => {
                tasks.requests.remove(request);
            }
            Msg::Timer(id) => {
                tasks.timers.remove(id);
            }
            _ => {}
        }
        let commands = update(&mut model, message);
        for (id, task) in &tasks.streams {
            task.cursor
                .send_replace(model.stream_cursors.get(id).cloned().unwrap_or_default());
        }
        if execute(&model, &client, terminal, &sender, commands, &mut tasks).await? {
            break;
        }
    }
    tasks.stop_all().await;
    Ok(())
}

async fn execute<B, C>(
    model: &Model,
    client: &C,
    terminal: &mut Terminal<B>,
    sender: &mpsc::UnboundedSender<Msg>,
    commands: Vec<Cmd>,
    tasks: &mut Tasks,
) -> Result<bool, String>
where
    B: Backend,
    B::Error: Display,
    C: RuntimeClient,
{
    for command in commands {
        match command {
            Cmd::Get(request) => {
                let id = request.id();
                let future = client.get(request);
                let sender = sender.clone();
                if let Some(old) = tasks.requests.insert(
                    id,
                    tokio::spawn(async move {
                        let result = future.await;
                        let _ = sender.send(Msg::Api {
                            request: id,
                            result,
                        });
                    }),
                ) {
                    old.abort();
                }
            }
            Cmd::Post(request) => {
                let id = request.id();
                let future = client.post(request);
                let sender = sender.clone();
                if let Some(old) = tasks.requests.insert(
                    id,
                    tokio::spawn(async move {
                        let result = future.await;
                        let _ = sender.send(Msg::Api {
                            request: id,
                            result,
                        });
                    }),
                ) {
                    old.abort();
                }
            }
            Cmd::StartStream(id) => {
                tasks.stop_stream(&id).await;
                let initial = model.stream_cursors.get(&id).cloned().unwrap_or_default();
                let (cursor_tx, cursor_rx) = watch::channel(initial);
                let error_only = matches!(
                    (&id, &model.overlay),
                    (
                        StreamId::Logs(stream_scope),
                        Some(crate::app::Overlay::Logs { scope, error_only:true, .. })
                    ) if stream_scope == scope
                );
                let mut events = client.stream_with_options(id.clone(), cursor_rx, error_only);
                let sender = sender.clone();
                let message_id = id.clone();
                let task = tokio::spawn(async move {
                    while let Some(event) = events.next().await {
                        if sender
                            .send(Msg::Stream {
                                id: message_id.clone(),
                                event,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                tasks.streams.insert(
                    id,
                    StreamTask {
                        task,
                        cursor: cursor_tx,
                    },
                );
            }
            Cmd::StopStream(id) => tasks.stop_stream(&id).await,
            Cmd::After(duration, id) => {
                if let Some(old) = tasks.timers.remove(&id) {
                    old.abort();
                    let _ = old.await;
                }
                let sender = sender.clone();
                tasks.timers.insert(
                    id,
                    tokio::spawn(async move {
                        tokio::time::sleep(duration).await;
                        let _ = sender.send(Msg::Timer(id));
                    }),
                );
            }
            Cmd::Render => {
                terminal
                    .draw(|frame| views::view(model, frame))
                    .map_err(|error| error.to_string())?;
            }
            Cmd::Quit => {
                tasks.stop_all().await;
                return Ok(true);
            }
            Cmd::Notify { body } => {
                use std::io::Write;
                let mut stdout = std::io::stdout().lock();
                let _ = stdout.write_all(&notify_bytes(&body));
                let _ = stdout.flush();
            }
            Cmd::CopyToClipboard(payload) => {
                use std::io::Write;
                // OSC 52 receivers commonly truncate near 100 KB; stay well under.
                let capped: String = payload.chars().take(64 * 1024).collect();
                let mut stdout = std::io::stdout().lock();
                let _ = stdout.write_all(&osc52_bytes(&capped));
                let _ = stdout.flush();
            }
        }
    }
    Ok(false)
}

/// BEL (legacy attention) followed by OSC 9 (desktop notification on
/// supporting terminals: kitty, wezterm, foot, iTerm2). ST terminator.
pub fn notify_bytes(body: &str) -> Vec<u8> {
    let mut bytes = vec![0x07];
    bytes.extend_from_slice(b"\x1b]9;");
    // strip control chars so a transcript-derived body cannot break the sequence
    bytes.extend(
        body.chars()
            .filter(|c| !c.is_control())
            .collect::<String>()
            .into_bytes(),
    );
    bytes.extend_from_slice(b"\x1b\\");
    bytes
}

/// OSC 52 clipboard write (`ESC ] 52 ; c ; <base64> ESC \`). `c` targets the
/// system clipboard; base64 payload per the xterm spec. ST terminator.
pub fn osc52_bytes(payload: &str) -> Vec<u8> {
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
    format!("\x1b]52;c;{encoded}\x1b\\").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppMode, Settings};
    use futures_util::future::pending;
    use ratatui::backend::TestBackend;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn ut_760_notify_bytes_emit_bell_and_osc9() {
        let bytes = super::notify_bytes("2 attention items");
        assert!(bytes.starts_with(b"\x07"));
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\x1b]9;2 attention items\x1b\\"));
    }

    #[test]
    fn ut_762_osc52_bytes_wrap_base64_payload() {
        let bytes = super::osc52_bytes("hello");
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text, "\x1b]52;c;aGVsbG8=\x1b\\");
    }

    #[derive(Clone, Default)]
    struct Fake {
        calls: Arc<AtomicUsize>,
        current: Arc<AtomicUsize>,
        max: Arc<AtomicUsize>,
        cursors: Arc<Mutex<Vec<String>>>,
    }
    struct Guard {
        current: Arc<AtomicUsize>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            self.current.fetch_sub(1, Ordering::SeqCst);
        }
    }
    impl RuntimeClient for Fake {
        fn get(&self, _: Request) -> BoxFuture<'static, ApiResult> {
            Box::pin(pending())
        }
        fn post(&self, _: Request) -> BoxFuture<'static, ApiResult> {
            Box::pin(pending())
        }
        fn stream(
            &self,
            _: StreamId,
            cursor: watch::Receiver<String>,
        ) -> BoxStream<'static, AnyStreamEvent> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.cursors.lock().unwrap().push(cursor.borrow().clone());
            let current = self.current.clone();
            let max = self.max.clone();
            Box::pin(
                async_stream::stream! {let now=current.fetch_add(1,Ordering::SeqCst)+1;max.fetch_max(now,Ordering::SeqCst);let _guard=Guard{current};pending::<()>().await;#[allow(unreachable_code)]{yield AnyStreamEvent::Catalog(compozy_client::StreamEvent::Reconnected);}},
            )
        }
    }
    fn harness() -> (
        Model,
        Fake,
        Terminal<TestBackend>,
        mpsc::UnboundedSender<Msg>,
        mpsc::UnboundedReceiver<Msg>,
        Tasks,
    ) {
        let model = Model::new(Settings::default(), AppMode::Full);
        let client = Fake::default();
        let terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let (sender, receiver) = mpsc::unbounded_channel();
        (model, client, terminal, sender, receiver, Tasks::default())
    }

    #[tokio::test]
    async fn ut_660_replacing_a_stream_aborts_before_connect() {
        let (model, client, mut terminal, sender, _receiver, mut tasks) = harness();
        execute(
            &model,
            &client,
            &mut terminal,
            &sender,
            vec![Cmd::StartStream(StreamId::Catalog)],
            &mut tasks,
        )
        .await
        .unwrap();
        tokio::task::yield_now().await;
        execute(
            &model,
            &client,
            &mut terminal,
            &sender,
            vec![Cmd::StartStream(StreamId::Catalog)],
            &mut tasks,
        )
        .await
        .unwrap();
        tokio::task::yield_now().await;
        assert_eq!(client.calls.load(Ordering::SeqCst), 2);
        assert_eq!(client.current.load(Ordering::SeqCst), 1);
        assert_eq!(client.max.load(Ordering::SeqCst), 1);
        tasks.stop_all().await;
    }
    #[tokio::test]
    async fn ut_661_different_streams_are_independent() {
        let (model, client, mut terminal, sender, _receiver, mut tasks) = harness();
        execute(
            &model,
            &client,
            &mut terminal,
            &sender,
            vec![
                Cmd::StartStream(StreamId::Catalog),
                Cmd::StartStream(StreamId::RunEvents("r".into())),
            ],
            &mut tasks,
        )
        .await
        .unwrap();
        tokio::task::yield_now().await;
        assert_eq!(client.current.load(Ordering::SeqCst), 2);
        execute(
            &model,
            &client,
            &mut terminal,
            &sender,
            vec![Cmd::StopStream(StreamId::Catalog)],
            &mut tasks,
        )
        .await
        .unwrap();
        assert_eq!(client.current.load(Ordering::SeqCst), 1);
        tasks.stop_all().await;
    }
    #[tokio::test(start_paused = true)]
    async fn ut_663_timer_rearm_replaces_old_timer() {
        let (model, client, mut terminal, sender, mut receiver, mut tasks) = harness();
        execute(
            &model,
            &client,
            &mut terminal,
            &sender,
            vec![
                Cmd::After(Duration::from_secs(10), TimerId::ToastExpiry),
                Cmd::After(Duration::from_secs(20), TimerId::ToastExpiry),
            ],
            &mut tasks,
        )
        .await
        .unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(10)).await;
        assert!(receiver.try_recv().is_err());
        tokio::time::advance(Duration::from_secs(10)).await;
        tokio::task::yield_now().await;
        assert!(matches!(
            receiver.try_recv(),
            Ok(Msg::Timer(TimerId::ToastExpiry))
        ));
        tasks.stop_all().await;
    }
    #[tokio::test]
    async fn ut_668_stream_receives_its_cursor_watch() {
        let (mut model, client, mut terminal, sender, _receiver, mut tasks) = harness();
        let id = StreamId::RunEvents("r".into());
        model.stream_cursors.insert(id.clone(), "42".into());
        execute(
            &model,
            &client,
            &mut terminal,
            &sender,
            vec![Cmd::StartStream(id)],
            &mut tasks,
        )
        .await
        .unwrap();
        assert_eq!(client.cursors.lock().unwrap().as_slice(), ["42"]);
        tasks.stop_all().await;
    }
    #[tokio::test]
    async fn ut_455_quit_aborts_a_pending_post_within_one_second() {
        let (model, client, mut terminal, sender, _receiver, mut tasks) = harness();
        let request = Request::CancelPrompt {
            id: RequestId(9),
            workspace: "ws".into(),
            session: "s".into(),
        };
        execute(
            &model,
            &client,
            &mut terminal,
            &sender,
            vec![Cmd::Post(request)],
            &mut tasks,
        )
        .await
        .unwrap();
        tokio::time::timeout(
            Duration::from_secs(1),
            execute(
                &model,
                &client,
                &mut terminal,
                &sender,
                vec![Cmd::Quit],
                &mut tasks,
            ),
        )
        .await
        .expect("quit did not wait for post")
        .unwrap();
    }
    #[test]
    fn ut_666_render_tick_uses_configured_fps() {
        assert_eq!(render_interval(10), Duration::from_millis(100));
        assert_eq!(render_interval(1), Duration::from_millis(200));
        assert_eq!(render_interval(100), Duration::from_nanos(16_666_667));
    }

    #[test]
    fn daemon_api_errors_retain_structured_fields_for_the_ui() {
        assert_eq!(
            api_error_text(compozy_client::Error::Daemon {
                status: 413,
                message: "input queue full".into(),
                code: Some("queue_full".into()),
                details: Some(serde_json::json!({"queue_cap": 10})),
                diagnostic: Some(serde_json::json!("wait for a queued request")),
            }),
            ApiError {
                message: "input queue full".into(),
                status: Some(413),
                code: Some("queue_full".into()),
                details: Some(serde_json::json!({"queue_cap": 10})),
                diagnostic: Some(serde_json::json!("wait for a queued request")),
            }
        );
    }
}
