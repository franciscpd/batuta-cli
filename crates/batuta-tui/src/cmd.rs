use compozy_client::{
    GateDecision, RunControl, TaskVerb,
    types::{ApproveRequest, ClarifyAnswer, PromptRequest},
};
use std::{fmt, time::Duration};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LogScope {
    Workspace,
    Session(String),
    Run(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StreamId {
    Transcript(String),
    Catalog,
    RunEvents(String),
    Logs(LogScope),
}

impl fmt::Display for StreamId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transcript(id) => write!(formatter, "transcript:{id}"),
            Self::Catalog => formatter.write_str("catalog"),
            Self::RunEvents(id) => write!(formatter, "run:{id}"),
            Self::Logs(LogScope::Workspace) => formatter.write_str("logs:workspace"),
            Self::Logs(LogScope::Session(id)) => write!(formatter, "logs:session:{id}"),
            Self::Logs(LogScope::Run(id)) => write!(formatter, "logs:run:{id}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimerId {
    StatusPoll,
    RunsPoll,
    AttentionPoll,
    CatalogDebounce,
    CatalogPoll,
    ToastExpiry,
    QuitGuard,
    DetailSwitchDebounce,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Status {
        id: RequestId,
    },
    Workspaces {
        id: RequestId,
    },
    Sessions {
        id: RequestId,
        workspace: String,
        agent: Option<String>,
        limit: u64,
    },
    Session {
        id: RequestId,
        workspace: String,
        session: String,
    },
    TranscriptPage {
        id: RequestId,
        workspace: String,
        session: String,
        before_sequence: Option<i64>,
    },
    VisibleTranscript {
        id: RequestId,
        workspace: String,
        session: String,
    },
    Runs {
        id: RequestId,
        workspace: String,
        loop_name: Option<String>,
        limit: u64,
    },
    Run {
        id: RequestId,
        workspace: String,
        run: String,
    },
    Overview {
        id: RequestId,
        workspace: String,
    },
    Logs {
        id: RequestId,
        workspace: String,
        scope: LogScope,
        error_only: bool,
        limit: u64,
    },
    Clarifications {
        id: RequestId,
        workspace: String,
        session: String,
    },
    CreateSession {
        id: RequestId,
        workspace: String,
        agent: String,
    },
    Prompt {
        id: RequestId,
        workspace: String,
        session: String,
        prompt: PromptRequest,
    },
    CancelPrompt {
        id: RequestId,
        workspace: String,
        session: String,
    },
    Approve {
        id: RequestId,
        workspace: String,
        session: String,
        request: ApproveRequest,
    },
    AnswerClarification {
        id: RequestId,
        workspace: String,
        session: String,
        request_id: String,
        answer: ClarifyAnswer,
    },
    RunControl {
        id: RequestId,
        workspace: String,
        run: String,
        control: RunControl,
    },
    RunApprove {
        id: RequestId,
        workspace: String,
        run: String,
        gate_id: String,
        decision: GateDecision,
    },
    TaskVerb {
        id: RequestId,
        workspace: String,
        task_id: String,
        verb: TaskVerb,
    },
}

impl Request {
    pub const fn id(&self) -> RequestId {
        match self {
            Self::Status { id }
            | Self::Workspaces { id }
            | Self::Sessions { id, .. }
            | Self::Session { id, .. }
            | Self::TranscriptPage { id, .. }
            | Self::VisibleTranscript { id, .. }
            | Self::Runs { id, .. }
            | Self::Run { id, .. }
            | Self::Overview { id, .. }
            | Self::Logs { id, .. }
            | Self::Clarifications { id, .. }
            | Self::CreateSession { id, .. }
            | Self::Prompt { id, .. }
            | Self::CancelPrompt { id, .. }
            | Self::Approve { id, .. }
            | Self::AnswerClarification { id, .. }
            | Self::RunControl { id, .. }
            | Self::RunApprove { id, .. }
            | Self::TaskVerb { id, .. } => *id,
        }
    }

    pub const fn is_write(&self) -> bool {
        matches!(
            self,
            Self::CreateSession { .. }
                | Self::Prompt { .. }
                | Self::CancelPrompt { .. }
                | Self::Approve { .. }
                | Self::AnswerClarification { .. }
                | Self::RunControl { .. }
                | Self::RunApprove { .. }
                | Self::TaskVerb { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cmd {
    Get(Request),
    Post(Request),
    StartStream(StreamId),
    StopStream(StreamId),
    After(Duration, TimerId),
    Render,
    Quit,
}
