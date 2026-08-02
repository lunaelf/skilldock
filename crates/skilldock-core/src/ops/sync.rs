use serde::Serialize;

use crate::error::Result;
use crate::lock::Lock;
use crate::skilldock::Skilldock;
use crate::{cache, git};

/// What `sync` did to the Cache.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SyncOutcome {
    /// Repos in the lock that were freshly cloned.
    pub cloned: Vec<String>,
    /// Repos already present that were (re-)checked-out to the locked SHA.
    pub updated: Vec<String>,
}

/// Make the Cache exactly match the lock: clone any missing Source repo and
/// check every clone out to its locked SHA, so any machine reproduces the same
/// skill versions.
pub fn sync(sd: &Skilldock) -> Result<SyncOutcome> {
    let lock = Lock::read(&sd.lock_path())?;
    let mut outcome = SyncOutcome::default();

    for repo in &lock.repos {
        let (clone_dir, freshly_cloned) = cache::ensure_clone(sd, &repo.repo, &repo.clone_url())?;
        git::checkout(&clone_dir, &repo.resolved)?;
        if freshly_cloned {
            outcome.cloned.push(repo.repo.clone());
        } else {
            outcome.updated.push(repo.repo.clone());
        }
    }

    Ok(outcome)
}
