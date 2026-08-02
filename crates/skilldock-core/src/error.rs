use std::path::PathBuf;

/// Errors surfaced by `skilldock-core` operations.
///
/// The CLI and GUI adapters map these onto their own presentation (exit codes,
/// dialogs); the library never prints or exits.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not locate a home directory; set SKILLDOCK_HOME to choose the skilldock root")]
    NoHome,

    #[error("failed to parse {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("failed to serialize {path}: {source}")]
    TomlWrite {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },

    /// A glob pattern reached the lock, which may only hold exact entries.
    #[error("lock entry '{0}' contains a glob metacharacter; the lock holds only exact paths")]
    GlobInLock(String),

    /// A shelled-out `git` command failed.
    #[error("git {command} failed: {stderr}")]
    Git { command: String, stderr: String },

    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Wrap an [`std::io::Error`] with the path it happened at.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
