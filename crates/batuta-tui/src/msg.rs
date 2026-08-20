use crate::cmd::{RequestId, StreamId, TimerId};
use compozy_client::{
    StreamEvent, TranscriptEvent,
    types::{
        AddWorkspaceOutcome, CatalogEvent, Clarification, ClarifyResult, LogEvent, LoopEvent,
        LoopMutation, LoopRunDetail, LoopRunPage, Overview, PromptOutcome, Session, SessionPage,
        StatusPayload, TranscriptPage, Workspace,
    },
};
use crossterm::event::{KeyEvent, MouseEvent};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct ApiError {
    pub message: String,
    pub status: Option<u16>,
    pub code: Option<String>,
    pub details: Option<Value>,
    pub diagnostic: Option<Value>,
}

impl ApiError {
    pub fn display_text(&self) -> String {
        self.status
            .map(|status| format!("HTTP {status}: {}", self.message))
            .unwrap_or_else(|| self.message.clone())
    }

    pub fn technical_details(&self) -> Option<String> {
        let mut lines = Vec::new();
        if let Some(code) = &self.code {
            lines.push(format!("Code: {code}"));
        }
        if let Some(diagnostic) = &self.diagnostic {
            lines.push(format!("Diagnostic: {diagnostic}"));
        }
        if let Some(details) = &self.details {
            lines.push(format!("Details: {details}"));
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}

impl From<&str> for ApiError {
    fn from(message: &str) -> Self {
        Self {
            message: message.into(),
            status: None,
            code: None,
            details: None,
            diagnostic: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ApiResponse {
    Status(Box<StatusPayload>),
    Workspaces(Vec<Workspace>),
    WorkspaceAdded(AddWorkspaceOutcome),
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

pub type ApiResult = Result<ApiResponse, ApiError>;

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
