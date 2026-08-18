use crate::cmd::{RequestId, StreamId, TimerId};
use compozy_client::{
    StreamEvent, TranscriptEvent,
    types::{
        CatalogEvent, Clarification, ClarifyResult, LogEvent, LoopEvent, LoopMutation,
        LoopRunDetail, LoopRunPage, Overview, PromptOutcome, Session, SessionPage, StatusPayload,
        TranscriptPage, Workspace,
    },
};
use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Clone, Debug)]
pub enum ApiResponse {
    Status(Box<StatusPayload>),
    Workspaces(Vec<Workspace>),
    Sessions(Box<SessionPage>),
    Session(Session),
    TranscriptPage(Box<TranscriptPage>),
    Runs(Box<LoopRunPage>),
    Run(Box<LoopRunDetail>),
    Overview(Box<Overview>),
    Logs(Vec<LogEvent>),
    Clarifications(Vec<Clarification>),
    SessionCreated(Session),
    Prompt(PromptOutcome),
    ClarificationAnswered(ClarifyResult),
    RunMutation(LoopMutation),
    Empty,
}

pub type ApiResult = Result<ApiResponse, String>;

#[derive(Debug)]
pub enum AnyStreamEvent {
    Transcript(TranscriptEvent),
    Catalog(StreamEvent<CatalogEvent>),
    Run(StreamEvent<LoopEvent>),
    Logs(StreamEvent<LogEvent>),
}

impl AnyStreamEvent {
    pub fn is_lost(&self) -> bool {
        matches!(
            self,
            Self::Transcript(TranscriptEvent::Lost { .. } | TranscriptEvent::Fatal(_))
                | Self::Catalog(StreamEvent::Lost { .. } | StreamEvent::Fatal(_))
                | Self::Run(StreamEvent::Lost { .. } | StreamEvent::Fatal(_))
                | Self::Logs(StreamEvent::Lost { .. } | StreamEvent::Fatal(_))
        )
    }

    pub fn is_live(&self) -> bool {
        matches!(
            self,
            Self::Transcript(
                TranscriptEvent::Snapshot(_)
                    | TranscriptEvent::Delta(_)
                    | TranscriptEvent::Reconnected
            ) | Self::Catalog(StreamEvent::Event(_) | StreamEvent::Reconnected)
                | Self::Run(StreamEvent::Event(_) | StreamEvent::Reconnected)
                | Self::Logs(StreamEvent::Event(_) | StreamEvent::Reconnected)
        )
    }
}

#[derive(Debug)]
pub enum Msg {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
    Api {
        request: RequestId,
        result: ApiResult,
    },
    Stream {
        id: StreamId,
        event: AnyStreamEvent,
    },
    Timer(TimerId),
    Quit,
}
