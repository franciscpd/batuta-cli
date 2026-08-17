use crate::{
    app::{Model, update},
    cmd::Cmd,
    msg::Msg,
    views,
};
use compozy_client::{
    Client, ReconnectPolicy, StreamCursor, TranscriptEvent, TranscriptQuery, types::TranscriptPage,
};
use crossterm::event::{Event, EventStream};
use futures_util::{StreamExt, future::BoxFuture, stream::BoxStream};
use ratatui::{Terminal, backend::Backend};
use std::{fmt::Display, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

#[derive(Default)]
struct Tasks {
    fetch: Option<JoinHandle<()>>,
    stream: Option<JoinHandle<()>>,
    cursor: Option<watch::Sender<StreamCursor>>,
}

impl Tasks {
    fn stop_stream(&mut self) {
        if let Some(task) = self.stream.take() {
            task.abort();
        }
        self.cursor = None;
    }

    fn stop_all(&mut self) {
        if let Some(task) = self.fetch.take() {
            task.abort();
        }
        self.stop_stream();
    }
}

pub trait RuntimeClient: Clone + Send + Sync + 'static {
    fn transcript_page(
        &self,
        workspace: String,
        session: String,
        before: Option<i64>,
    ) -> BoxFuture<'static, Result<TranscriptPage, String>>;
    fn transcript_events(
        &self,
        workspace: String,
        session: String,
        cursor: watch::Receiver<StreamCursor>,
    ) -> BoxStream<'static, TranscriptEvent>;
}

impl RuntimeClient for Client {
    fn transcript_page(
        &self,
        workspace: String,
        session: String,
        before: Option<i64>,
    ) -> BoxFuture<'static, Result<TranscriptPage, String>> {
        let client = self.clone();
        Box::pin(async move {
            client
                .transcript(
                    &workspace,
                    &session,
                    &TranscriptQuery {
                        limit: 200,
                        before_sequence: before,
                    },
                )
                .await
                .map_err(|error| error.to_string())
        })
    }

    fn transcript_events(
        &self,
        workspace: String,
        session: String,
        cursor: watch::Receiver<StreamCursor>,
    ) -> BoxStream<'static, TranscriptEvent> {
        Box::pin(self.transcript_stream(&workspace, &session, cursor, ReconnectPolicy::default()))
    }
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
                Ok(_) => continue,
                Err(error) => {
                    tracing::warn!(%error, "terminal event stream failed");
                    break;
                }
            };
            if event_sender.send(message).is_err() {
                break;
            }
        }
    });
    let tick_sender = sender.clone();
    let tick_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(33));
        loop {
            interval.tick().await;
            if tick_sender.send(Msg::Tick).is_err() {
                break;
            }
        }
    });
    let result = run_with_messages(model, client, terminal, sender, receiver).await;
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
    if execute_commands(&model, &client, terminal, &sender, initial, &mut tasks).await? {
        return Ok(());
    }
    while let Some(message) = receiver.recv().await {
        let commands = update(&mut model, message);
        if let Some(cursor_sender) = &tasks.cursor {
            cursor_sender.send_replace(model.transcript.cursor());
        }
        if execute_commands(&model, &client, terminal, &sender, commands, &mut tasks).await? {
            break;
        }
    }
    tasks.stop_all();
    Ok(())
}

async fn execute_commands<B, C>(
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
            Cmd::FetchPage { before_sequence } => {
                if tasks.fetch.as_ref().is_some_and(|task| !task.is_finished()) {
                    continue;
                }
                let client = client.clone();
                let workspace = model.header.workspace_id.clone();
                let session = model.header.session_id.clone();
                let sender = sender.clone();
                tasks.fetch = Some(tokio::spawn(async move {
                    let result = client
                        .transcript_page(workspace, session, before_sequence)
                        .await;
                    let _ = sender.send(Msg::Page(result));
                }));
            }
            Cmd::StartStream(cursor) => {
                tasks.stop_stream();
                let (new_cursor_sender, cursor_receiver) = watch::channel(cursor);
                tasks.cursor = Some(new_cursor_sender);
                let client = client.clone();
                let workspace = model.header.workspace_id.clone();
                let session = model.header.session_id.clone();
                let sender = sender.clone();
                tasks.stream = Some(tokio::spawn(async move {
                    let mut events = client.transcript_events(workspace, session, cursor_receiver);
                    while let Some(event) = events.next().await {
                        if sender.send(Msg::Stream(event)).is_err() {
                            break;
                        }
                    }
                }));
            }
            Cmd::StopStream => {
                tasks.stop_stream();
            }
            Cmd::ClearFooterAfter(duration) => {
                let sender = sender.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(duration).await;
                    let _ = sender.send(Msg::ClearFooter);
                });
            }
            Cmd::Render => {
                terminal
                    .draw(|frame| views::view(model, frame))
                    .map_err(|error| error.to_string())?;
                tracing::trace!(entries = model.transcript.len(), "tail.render");
            }
            Cmd::Quit => {
                tasks.stop_all();
                return Ok(true);
            }
        }
    }
    Ok(false)
}
