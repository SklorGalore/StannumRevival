//! The errors Stannum's commands return.
//!
//! Every error carries a [`ErrorKind`] a client can act on (a script can
//! catch "cancelled" or "no case" without reading the text) and a message
//! written for the person using the app. Over JSON an error reads
//! `{"kind": "no_case", "message": "Open a case first."}`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

/// What went wrong, in a form a client can match on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// No command has this name.
    UnknownCommand,
    /// The arguments are missing, misspelled or the wrong type, or a value
    /// isn't one the command takes.
    InvalidArguments,
    /// The command works on a case and none is open.
    NoCase,
    /// The `case` argument names a case that isn't open (it was closed or
    /// replaced).
    CaseNotOpen,
    /// Another step has to come first, such as solving the case before
    /// asking for its results.
    NotReady,
    /// The element, record type, result or row asked for isn't there.
    NotFound,
    /// The command can't work on this case's format or revision.
    Unsupported,
    /// The case changed while the work ran, so its results were set aside.
    /// Running it again works.
    CaseChanged,
    /// The work was stopped on request.
    Cancelled,
    /// A file couldn't be read or written.
    File,
    /// The work itself failed: a file didn't parse, a solve couldn't start,
    /// an edit was refused. The message says why.
    Failed,
    /// Something inside Stannum went wrong. Please report it.
    Internal,
}

impl ErrorKind {
    /// Every kind, in the order declared.
    pub const ALL: [ErrorKind; 12] = [
        Self::UnknownCommand,
        Self::InvalidArguments,
        Self::NoCase,
        Self::CaseNotOpen,
        Self::NotReady,
        Self::NotFound,
        Self::Unsupported,
        Self::CaseChanged,
        Self::Cancelled,
        Self::File,
        Self::Failed,
        Self::Internal,
    ];

    /// The kind as it reads in JSON ("no_case").
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownCommand => "unknown_command",
            Self::InvalidArguments => "invalid_arguments",
            Self::NoCase => "no_case",
            Self::CaseNotOpen => "case_not_open",
            Self::NotReady => "not_ready",
            Self::NotFound => "not_found",
            Self::Unsupported => "unsupported",
            Self::CaseChanged => "case_changed",
            Self::Cancelled => "cancelled",
            Self::File => "file",
            Self::Failed => "failed",
            Self::Internal => "internal",
        }
    }
}

/// An error from a command: what kind it is, and what to tell the person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidArguments, message)
    }

    pub fn not_ready(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotReady, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unsupported, message)
    }

    pub fn cancelled(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Cancelled, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    /// A file that couldn't be read or written, named in the message.
    pub fn file(path: &Path, error: std::io::Error) -> Self {
        Self::new(ErrorKind::File, format!("{}: {error}", path.display()))
    }

    pub fn is(&self, kind: ErrorKind) -> bool {
        self.kind == kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// Errors from the parts of the core that report text only are failures.
impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::new(ErrorKind::Failed, message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::new(ErrorKind::Failed, message)
    }
}

/// For code that still reports text: the message.
impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.message
    }
}

/// Compares the message, so tests can check the text directly.
impl PartialEq<str> for Error {
    fn eq(&self, other: &str) -> bool {
        self.message == other
    }
}

impl PartialEq<&str> for Error {
    fn eq(&self, other: &&str) -> bool {
        self.message == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_as_kind_and_message() {
        let e = Error::new(ErrorKind::NoCase, "Open a case first.");
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"kind":"no_case","message":"Open a case first."}"#
        );
        assert_eq!(e.to_string(), "Open a case first.");
    }

    #[test]
    fn kind_names_match_their_json() {
        for kind in ErrorKind::ALL {
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::Value::String(kind.as_str().into())
            );
        }
    }
}
