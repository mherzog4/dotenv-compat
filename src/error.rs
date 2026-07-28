use std::fmt;
use std::io::ErrorKind;
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
    /// `DOTENV_KEY` is set and a `.env.vault` exists, but this crate cannot
    /// decrypt it.
    ///
    /// The reference would AES-256-GCM decrypt the vault here. Doing that needs
    /// a crypto dependency, so rather than silently loading a plain `.env` --
    /// which is a different set of secrets -- the load is refused.
    VaultUnsupported { path: PathBuf },
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
            Error::VaultUnsupported { .. } => ErrorKind::Unsupported,
        }
    }

    /// The file this error is about.
    pub fn path(&self) -> &PathBuf {
        match self {
            Error::Io { path, .. } | Error::VaultUnsupported { path } => path,
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
            Error::VaultUnsupported { path } => write!(
                f,
                "DOTENV_KEY is set and {} exists, but dotenv-compat cannot decrypt .env.vault files",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            Error::VaultUnsupported { .. } => None,
        }
    }
}
