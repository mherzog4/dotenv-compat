use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// A failure encountered while loading a `.env` file.
///
/// Mirrors the `error` field the reference implementation puts on its config
/// result: a missing or unreadable file is reported, never fatal.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The file could not be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The `DOTENV_KEY` is malformed. `INVALID_DOTENV_KEY` in JavaScript.
    InvalidDotenvKey(String),
    /// The vault could not be decrypted with any supplied key.
    /// `DECRYPTION_FAILED` in JavaScript.
    DecryptionFailed,
    /// The vault has no entry for the environment named by the `DOTENV_KEY`.
    /// `NOT_FOUND_DOTENV_ENVIRONMENT` in JavaScript.
    NotFoundDotenvEnvironment(String),
    /// The vault file could not be parsed. `MISSING_DATA` in JavaScript.
    MissingData(String),
    /// An unrecognised `encoding` name. `fs.readFileSync` throws a `TypeError`
    /// with code `ERR_INVALID_ARG_VALUE` here.
    InvalidEncoding(String),
}

impl Error {
    /// The underlying I/O error kind.
    ///
    /// The reference returns Node's own error, so callers match on
    /// `err.code === 'ENOENT'`. The Rust equivalent is
    /// `matches!(err.kind(), ErrorKind::NotFound)`; the message text itself
    /// cannot match Node's, since it comes from the platform.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Io { source, .. } => source.kind(),
            _ => ErrorKind::Other,
        }
    }

    /// The JavaScript `err.code`, for the errors that carry one.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Error::Io { source, .. } => match source.kind() {
                ErrorKind::NotFound => Some("ENOENT"),
                ErrorKind::PermissionDenied => Some("EACCES"),
                ErrorKind::IsADirectory => Some("EISDIR"),
                ErrorKind::NotADirectory => Some("ENOTDIR"),
                _ => None,
            },
            Error::InvalidDotenvKey(_) => Some("INVALID_DOTENV_KEY"),
            Error::DecryptionFailed => Some("DECRYPTION_FAILED"),
            Error::NotFoundDotenvEnvironment(_) => Some("NOT_FOUND_DOTENV_ENVIRONMENT"),
            Error::MissingData(_) => Some("MISSING_DATA"),
            Error::InvalidEncoding(_) => Some("ERR_INVALID_ARG_VALUE"),
        }
    }

    /// The file this error is about, when it is about one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Error::Io { path, .. } => Some(path),
            _ => None,
        }
    }
}

/// Render an I/O error the way Node's `fs` does: `ENOENT: no such file or
/// directory, open 'x.env'`.
///
/// The reference puts Node's own `Error` on the result, and callers are
/// documented to match on `err.code === 'ENOENT'`, so the text is reproduced.
/// Use [`Error::kind`] rather than parsing it.
pub(crate) fn node_message(path: &std::path::Path, source: &std::io::Error) -> String {
    let code = match source.kind() {
        ErrorKind::NotFound => "ENOENT",
        ErrorKind::PermissionDenied => "EACCES",
        ErrorKind::IsADirectory => "EISDIR",
        ErrorKind::NotADirectory => "ENOTDIR",
        ErrorKind::AlreadyExists => "EEXIST",
        _ => return format!("{source}, open '{}'", path.display()),
    };

    // Rust renders "No such file or directory (os error 2)"; Node renders
    // "no such file or directory".
    let rendered = source.to_string();
    let text = rendered.split(" (os error").next().unwrap_or(&rendered);
    let mut chars = text.chars();
    let lowered = match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    };

    format!("{code}: {lowered}, open '{}'", path.display())
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => f.write_str(&node_message(path, source)),
            // Message text matches the reference exactly.
            Error::InvalidDotenvKey(detail) => write!(f, "INVALID_DOTENV_KEY: {detail}"),
            Error::DecryptionFailed => {
                f.write_str("DECRYPTION_FAILED: Please check your DOTENV_KEY")
            }
            Error::NotFoundDotenvEnvironment(detail) => {
                write!(f, "NOT_FOUND_DOTENV_ENVIRONMENT: {detail}")
            }
            Error::MissingData(detail) => write!(f, "MISSING_DATA: {detail}"),
            Error::InvalidEncoding(name) => write!(
                f,
                "The argument 'encoding' is invalid encoding. Received '{name}'"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
