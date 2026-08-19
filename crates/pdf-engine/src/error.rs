use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineErrorKind {
    InvalidDocument,
    PasswordRequired,
    IncorrectPassword,
    Unsupported,
    ResourceLimit,
    Rendering,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineError {
    kind: EngineErrorKind,
    message: String,
}

impl EngineError {
    pub fn new(kind: EngineErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> EngineErrorKind {
        self.kind
    }
}

impl Display for EngineError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EngineError {}
