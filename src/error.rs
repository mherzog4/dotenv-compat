use std::fmt;
use std::path::PathBuf;

/// A failure encountered while loading a `.env` file.
///
/// Mirrors the `error` field the reference implementation puts on its config
/// result: a missing or unreadable file is reported, never fatal.
#[derive(Debug)]
pub enum Error {
    /// The file could not be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => write!(f, "failed to load {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
        }
    }
}
