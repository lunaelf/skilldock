use crate::error::Result;
use crate::lock::{Lock, LockRepo, LockSkill};
use crate::manifest::{Manifest, SkillSpec};
use crate::skilldock::Skilldock;
use crate::source::Source;
use crate::vendored;

/// A request to declare and resolve a vendored source repo.
#[derive(Debug, Clone)]
pub struct AddRequest {
    /// The source: its canonical identity and clone URL (kept bundled).
    pub source: Source,
    /// Branch or tag to pin; `None` means the clone's default branch.
    pub git_ref: Option<String>,
    /// Declared skill specs: bare paths, globs, or `{name, path}` renames.
    pub skills: Vec<SkillSpec>,
}

/// What `add` resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddOutcome {
    pub repo: String,
    /// The pinned commit SHA.
    pub resolved: String,
    /// The concrete, hashed skills written to the lock.
    pub skills: Vec<LockSkill>,
}

/// Declare a vendored source, clone it into the Cache, pin it to one commit,
/// expand its declared skills, and record everything in the manifest and lock.
pub fn add(sd: &Skilldock, req: AddRequest) -> Result<AddOutcome> {
    let Source { repo, url } = &req.source;

    // Clone/refresh the Source, pin it to one commit, and expand its skills.
    let res = vendored::resolve(sd, repo, url, req.git_ref.as_deref(), &req.skills)?;

    // Record the declaration (manifest) and the resolution (lock).
    let mut manifest = Manifest::read(&sd.manifest_path())?;
    manifest.declare_vendored(repo, req.git_ref.clone(), &req.skills);
    manifest.write(&sd.manifest_path())?;

    let mut lock = Lock::read(&sd.lock_path())?;
    lock.upsert_repo(LockRepo {
        repo: repo.clone(),
        url: url.clone(),
        resolved: res.resolved.clone(),
        skills: res.skills.clone(),
    });
    lock.write(&sd.lock_path())?;

    Ok(AddOutcome {
        repo: repo.clone(),
        resolved: res.resolved,
        skills: res.skills,
    })
}
