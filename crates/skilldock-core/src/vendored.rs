//! Shared vendored-resolution: clone/refresh a Source, pin it to one commit,
//! and expand its declared skills into hashed lock entries. `add` and `update`
//! both go through [`resolve`] so a pin is computed one way.

use crate::error::Result;
use crate::lock::LockSkill;
use crate::manifest::SkillSpec;
use crate::skilldock::Skilldock;
use crate::{cache, expand, git};

/// The result of pinning a Source: its commit and expanded skills.
pub struct Resolution {
    pub resolved: String,
    pub skills: Vec<LockSkill>,
}

/// Ensure the Cache clone exists (fetching if it was already present), move it
/// onto `git_ref` (or leave the default branch), pin `HEAD`, and expand `specs`.
pub fn resolve(
    sd: &Skilldock,
    repo: &str,
    url: &str,
    git_ref: Option<&str>,
    specs: &[SkillSpec],
) -> Result<Resolution> {
    let (clone_dir, freshly_cloned) = cache::ensure_clone(sd, repo, url)?;
    if !freshly_cloned {
        git::fetch(&clone_dir)?;
    }
    if let Some(git_ref) = git_ref {
        git::checkout(&clone_dir, git_ref)?;
    }
    let resolved = git::rev_parse(&clone_dir, "HEAD")?;
    let skills = expand::expand_skills(&clone_dir, specs)?;
    Ok(Resolution { resolved, skills })
}
