#[derive(Debug)]
pub struct AppError {
    message: String,
    code: i32,
    reported: bool,
}

impl AppError {
    pub fn daemon(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 1,
            reported: false,
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
            reported: false,
        }
    }

    pub fn reported(code: i32) -> Self {
        Self {
            message: String::new(),
            code,
            reported: true,
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.code
    }
    pub fn was_reported(&self) -> bool {
        self.reported
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl From<compozy_client::Error> for AppError {
    fn from(value: compozy_client::Error) -> Self {
        Self::daemon(value.to_string())
    }
}
