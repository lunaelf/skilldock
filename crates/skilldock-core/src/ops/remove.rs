use serde::Serialize;

use crate::cache;
use crate::error::{Error, Result};
use crate::lock::Lock;
use crate::manifest::Manifest;
use crate::skilldock::Skilldock;

/// What `remove` did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RemoveOutcome {
    /// The skill name or repo identity removed.
    pub removed: Vec<String>,
    /// Repo identities whose Cache clone was pruned (no longer referenced).
    pub pruned_clones: Vec<String>,
}

/// Remove a vendored skill or a whole repo from the manifest and lock, pruning
/// the Cache clone once no skill references it. Completes the removal the old
/// tool left half-done (files gone, lock entry lingering).
pub fn remove(sd: &Skilldock, input: &str) -> Result<RemoveOutcome> {
    let mut manifest = Manifest::read(&sd.manifest_path())?;
    let mut lock = Lock::read(&sd.lock_path())?;
    let mut outcome = RemoveOutcome::default();

    if input.contains('/') {
        remove_repo(sd, &mut manifest, &mut lock, input, &mut outcome)?;
    } else {
        remove_skill(sd, &mut manifest, &mut lock, input, &mut outcome)?;
    }

    manifest.write(&sd.manifest_path())?;
    lock.write(&sd.lock_path())?;
    Ok(outcome)
}

/// Remove an entire vendored repo (identity form).
fn remove_repo(
    sd: &Skilldock,
    manifest: &mut Manifest,
    lock: &mut Lock,
    repo: &str,
    outcome: &mut RemoveOutcome,
) -> Result<()> {
    let present = manifest.vendored.iter().any(|v| v.repo == repo)
        || lock.repos.iter().any(|r| r.repo == repo);
    if !present {
        return Err(Error::Invalid(format!("no vendored repo '{repo}'")));
    }
    drop_repo(sd, manifest, lock, repo, outcome)?;
    outcome.removed.push(repo.to_string());
    Ok(())
}

/// Drop a repo entry from manifest + lock and prune its Cache clone.
fn drop_repo(
    sd: &Skilldock,
    manifest: &mut Manifest,
    lock: &mut Lock,
    repo: &str,
    outcome: &mut RemoveOutcome,
) -> Result<()> {
    manifest.remove_vendored_repo(repo);
    lock.remove_repo(repo);
    cache::remove_clone(sd, repo)?;
    outcome.pruned_clones.push(repo.to_string());
    Ok(())
}

/// Remove a single vendored skill by name.
fn remove_skill(
    sd: &Skilldock,
    manifest: &mut Manifest,
    lock: &mut Lock,
    name: &str,
    outcome: &mut RemoveOutcome,
) -> Result<()> {
    let repo_id = match lock.repo_of_skill(name)? {
        Some(r) => r,
        None => {
            if manifest.authored.iter().any(|a| a == name) {
                return Err(Error::Invalid(format!(
                    "'{name}' is an authored skill; remove it from the Store directly"
                )));
            }
            return Err(Error::Invalid(format!(
                "no vendored skill or repo '{name}'"
            )));
        }
    };

    // Drop the explicit declaration from the manifest. A glob-provided skill has
    // no exact spec to remove — direct the user at the repo instead.
    let repo = manifest.vendored_repo_mut(&repo_id).ok_or_else(|| {
        Error::Invalid(format!(
            "'{name}' is locked but '{repo_id}' is not declared"
        ))
    })?;
    let before = repo.skills.len();
    repo.skills
        .retain(|s| s.link_name().as_deref() != Some(name));
    if repo.skills.len() == before {
        return Err(Error::Invalid(format!(
            "skill '{name}' is provided by a glob in '{repo_id}'; remove the whole repo '{repo_id}' or narrow the glob"
        )));
    }
    let repo_now_empty = repo.skills.is_empty();

    if let Some(locked) = lock.repo_mut(&repo_id) {
        locked.skills.retain(|s| s.name != name);
    }
    outcome.removed.push(name.to_string());

    // Last skill gone -> drop the repo and prune its clone.
    if repo_now_empty {
        drop_repo(sd, manifest, lock, &repo_id, outcome)?;
    }
    Ok(())
}
