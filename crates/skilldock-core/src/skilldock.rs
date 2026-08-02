use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The environment variable that relocates the whole skilldock (ADR-0003, story 24).
pub const HOME_ENV: &str = "SKILLDOCK_HOME";

/// A resolved handle to a per-user Skilldock rooted at `~/.skilldock`
/// (or `$SKILLDOCK_HOME`) — the whole system: Store, Cache, and config.
///
/// A `Skilldock` is just a set of paths; constructing one touches no filesystem.
/// Operations take a `&Skilldock` explicitly rather than reading the environment,
/// so tests can point at a throwaway root without mutating process-global state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skilldock {
    root: PathBuf,
}

impl Skilldock {
    /// Build a handle at an explicit root. Used by tests and callers that
    /// already know where the skilldock lives.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Skilldock { root: root.into() }
    }

    /// Resolve the root from the environment: `$SKILLDOCK_HOME` if set,
    /// otherwise `~/.skilldock`.
    pub fn from_env() -> Result<Self> {
        Self::resolve(std::env::var_os(HOME_ENV), dirs::home_dir())
    }

    /// The pure resolution rule behind [`from_env`](Self::from_env): a non-empty
    /// `$SKILLDOCK_HOME` wins, otherwise `<home>/.skilldock`. Extracted so the
    /// rule is tested without mutating the process environment.
    fn resolve(home_env: Option<OsString>, home_dir: Option<PathBuf>) -> Result<Self> {
        match home_env {
            Some(v) if !v.is_empty() => Ok(Skilldock::at(PathBuf::from(v))),
            _ => {
                let home = home_dir.ok_or(Error::NoHome)?;
                Ok(Skilldock::at(home.join(".skilldock")))
            }
        }
    }

    /// The root directory (`~/.skilldock`).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The data repo checkout: authored skills + manifest.
    pub fn store(&self) -> PathBuf {
        self.root.join("store")
    }

    /// The vendored clone tree: `cache/<host>/<owner>/<repo>`.
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// The config file (data-repo remote + prefs).
    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// The declared manifest inside the Store.
    pub fn manifest_path(&self) -> PathBuf {
        self.store().join("skilldock.toml")
    }

    /// The resolved lock inside the Store.
    pub fn lock_path(&self) -> PathBuf {
        self.store().join("skilldock.lock")
    }

    /// Directory holding authored skill originals inside the Store.
    pub fn authored_skills_dir(&self) -> PathBuf {
        self.store().join("skills")
    }

    /// The original directory of one authored skill.
    pub fn authored_skill_dir(&self, name: &str) -> PathBuf {
        self.authored_skills_dir().join(name)
    }

    /// The cache clone directory for a vendored source repo, e.g.
    /// `cache/github.com/mattpocock/skills`.
    pub fn cache_clone_dir(&self, repo: &str) -> PathBuf {
        let mut dir = self.cache();
        for seg in repo.split('/') {
            dir.push(seg);
        }
        dir
    }

    /// Create the skeleton (`store/skills`, `cache/`). Idempotent.
    pub fn ensure_layout(&self) -> Result<()> {
        for dir in [self.authored_skills_dir(), self.cache()] {
            std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        let sd = Skilldock::resolve(Some("/custom/root".into()), Some("/home/u".into())).unwrap();
        assert_eq!(sd.root(), Path::new("/custom/root"));
    }

    #[test]
    fn empty_override_falls_back_to_default() {
        let sd = Skilldock::resolve(Some(OsString::new()), Some("/home/u".into())).unwrap();
        assert_eq!(sd.root(), Path::new("/home/u/.skilldock"));
    }

    #[test]
    fn no_override_uses_home_default() {
        let sd = Skilldock::resolve(None, Some("/home/u".into())).unwrap();
        assert_eq!(sd.root(), Path::new("/home/u/.skilldock"));
    }

    #[test]
    fn no_override_and_no_home_errors() {
        let err = Skilldock::resolve(None, None).unwrap_err();
        assert!(matches!(err, Error::NoHome));
    }
}
