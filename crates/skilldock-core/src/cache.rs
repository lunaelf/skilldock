//! The vendored Cache: cloned Source repos under `cache/<host>/<owner>/<repo>`,
//! shared by `add` (resolve a new pin) and `sync` (reproduce from the lock).

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::git;
use crate::skilldock::Skilldock;

/// Ensure a Cache clone of `repo` exists, cloning from `url` if missing.
///
/// Returns the clone directory and whether it was freshly cloned (so callers
/// can skip a redundant fetch on a brand-new clone).
pub fn ensure_clone(sd: &Skilldock, repo: &str, url: &str) -> Result<(PathBuf, bool)> {
    let dir = sd.cache_clone_dir(repo);
    if dir.join(".git").is_dir() {
        return Ok((dir, false));
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    git::clone(url, &dir)?;
    Ok((dir, true))
}
