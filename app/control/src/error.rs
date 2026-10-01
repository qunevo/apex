use crate::model::Diagnostic;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Operation failures with stable meanings for every interface.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    /// The caller's expected revision or state is no longer current.
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid request: {message}")]
    Invalid {
        message: String,
        diagnostics: Vec<Diagnostic>,
    },
    #[error("storage failure: {0}")]
    Store(String),
}

impl Error {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
            diagnostics: Vec::new(),
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "NOT_FOUND",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::Conflict(_) => "CONFLICT",
            Self::Invalid { .. } => "INVALID",
            Self::Store(_) => "STORE",
        }
    }
}
