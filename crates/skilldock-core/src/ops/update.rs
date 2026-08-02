use serde::Serialize;

use crate::error::{Error, Result};
use crate::lock::{Lock, LockRepo};
use crate::manifest::{Manifest, VendoredRepo};
use crate::skilldock::Skilldock;
use crate::{cache, source, vendored};

/// One repo's before/after commit under `update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RepoUpdate {
    pub repo: String,
    /// The previously locked SHA, if any.
    pub from: Option<String>,
    /// The freshly resolved SHA.
    pub to: String,
    /// Whether the commit actually changed.
    pub moved: bool,
}

/// What `update` did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct UpdateOutcome {
    pub repos: Vec<RepoUpdate>,
}

/// Re-resolve declared refs to fresh commits, re-fetch the Cache, and rewrite
/// the lock (new SHA + re-hashed skills). With no `repos`, updates every
/// declared vendored source; otherwise only the named ones.
pub fn update(sd: &Skilldock, repos: &[String]) -> Result<UpdateOutcome> {
    let manifest = Manifest::read(&sd.manifest_path())?;
    let mut lock = Lock::read(&sd.lock_path())?;

    let targets = select_targets(&manifest, repos)?;
    let mut outcome = UpdateOutcome::default();

    for v in &targets {
        let locked = lock.repos.iter().find(|r| r.repo == v.repo);
        let from = locked.map(|r| r.resolved.clone());
        let url = match locked {
            Some(r) => r.clone_url(),
            None => source::clone_url_for(&v.repo),
        };

        // Re-clone fresh so a branch ref genuinely moves to its new tip.
        cache::remove_clone(sd, &v.repo)?;
        let res = vendored::resolve(sd, &v.repo, &url, v.git_ref.as_deref(), &v.skills)?;

        let moved = from.as_deref() != Some(res.resolved.as_str());
        lock.upsert_repo(LockRepo {
            repo: v.repo.clone(),
            url,
            resolved: res.resolved.clone(),
            skills: res.skills,
        });
        outcome.repos.push(RepoUpdate {
            repo: v.repo.clone(),
            from,
            to: res.resolved,
            moved,
        });
    }

    lock.write(&sd.lock_path())?;
    Ok(outcome)
}

/// The declared repos to update: all of them, or the named subset (erroring on
/// any name that isn't declared).
fn select_targets(manifest: &Manifest, repos: &[String]) -> Result<Vec<VendoredRepo>> {
    if repos.is_empty() {
        return Ok(manifest.vendored.clone());
    }
    let mut selected = Vec::new();
    for id in repos {
        let v = manifest
            .vendored
            .iter()
            .find(|v| v.repo == *id)
            .ok_or_else(|| Error::Invalid(format!("'{id}' is not a declared vendored repo")))?;
        selected.push(v.clone());
    }
    Ok(selected)
}
