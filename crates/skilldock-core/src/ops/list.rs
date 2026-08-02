use serde::Serialize;

use crate::error::Result;
use crate::lock::Lock;
use crate::manifest::Manifest;
use crate::skilldock::Skilldock;

/// A skilldock inventory grouped by provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Listing {
    pub authored: Vec<AuthoredSkill>,
    pub vendored: Vec<VendoredSkill>,
}

/// An authored skill as seen by `list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthoredSkill {
    pub name: String,
    /// Whether the original directory exists in the Store.
    pub present: bool,
}

/// A vendored skill as seen by `list` (resolved from the lock).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VendoredSkill {
    pub name: String,
    pub repo: String,
    pub path: String,
    /// The pinned commit SHA of the owning repo.
    pub resolved: String,
}

/// List the skills in the dock, grouped by provenance.
///
/// Authored skills come from the manifest's `authored` list; vendored skills
/// come from the lock (the resolved, exact truth). Both groups are sorted by
/// name for stable output.
pub fn list(sd: &Skilldock) -> Result<Listing> {
    let manifest = Manifest::read(&sd.manifest_path())?;
    let lock = Lock::read(&sd.lock_path())?;

    let mut authored: Vec<AuthoredSkill> = manifest
        .authored
        .iter()
        .map(|name| AuthoredSkill {
            present: sd.authored_skill_dir(name).is_dir(),
            name: name.clone(),
        })
        .collect();
    authored.sort_by(|a, b| a.name.cmp(&b.name));

    let mut vendored: Vec<VendoredSkill> = lock
        .repos
        .iter()
        .flat_map(|repo| {
            repo.skills.iter().map(move |skill| VendoredSkill {
                name: skill.name.clone(),
                repo: repo.repo.clone(),
                path: skill.path.clone(),
                resolved: repo.resolved.clone(),
            })
        })
        .collect();
    vendored.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Listing { authored, vendored })
}
