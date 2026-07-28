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
            Error::Io { source, .. } => node_code(source),
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
/// Use [`Error::code`] rather than parsing it.
///
/// The message strings are libuv's canonical ones rather than the platform's,
/// which is what makes them identical on every OS. Deriving them from Rust's
/// `Display` looked right on Unix but produced "The filename, directory name, or
/// volume label syntax is incorrect." on Windows, where Node still says ENOENT.
pub(crate) fn node_message(path: &std::path::Path, source: &std::io::Error) -> String {
    match node_code(source) {
        // `readFileSync` opens a directory successfully and fails on the read,
        // so this one names the syscall instead of the path.
        Some("EISDIR") => "EISDIR: illegal operation on a directory, read".to_string(),
        Some(code) => format!("{code}: {}, open '{}'", node_strerror(code), path.display()),
        None => format!("{source}, open '{}'", path.display()),
    }
}

/// libuv's `uv_translate_sys_error`, for the codes `fs.readFileSync` can raise.
pub(crate) fn node_code(source: &std::io::Error) -> Option<&'static str> {
    // libuv folds several Windows errors that Rust keeps distinct into ENOENT.
    // ERROR_INVALID_NAME (123) and ERROR_BAD_PATHNAME (161) are the ones a bad
    // path string reaches.
    if cfg!(windows) && matches!(source.raw_os_error(), Some(123) | Some(161)) {
        return Some("ENOENT");
    }

    match source.kind() {
        ErrorKind::NotFound => Some("ENOENT"),
        ErrorKind::PermissionDenied => Some("EACCES"),
        ErrorKind::IsADirectory => Some("EISDIR"),
        ErrorKind::NotADirectory => Some("ENOTDIR"),
        ErrorKind::AlreadyExists => Some("EEXIST"),
        _ => None,
    }
}

fn node_strerror(code: &str) -> &'static str {
    match code {
        "ENOENT" => "no such file or directory",
        "EACCES" => "permission denied",
        "ENOTDIR" => "not a directory",
        "EEXIST" => "file already exists",
        _ => "unknown error",
    }
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
