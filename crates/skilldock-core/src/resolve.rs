//! Resolve skill references to their Source directories.
//!
//! A reference is either a skill **name** (authored, or a vendored skill) or a
//! **repo identity** (`host/owner/repo`, expanded to all its vendored skills).
//! The authoritative source of each skill is its Source: the Store for authored,
//! the Cache clone for vendored.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::lock::Lock;
use crate::manifest::Manifest;
use crate::skilldock::Skilldock;

/// Where a skill's original lives (CONTEXT.md: the only essential distinction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    Vendored,
    Authored,
}

/// A skill resolved to a concrete Source directory, ready to link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLink {
    pub name: String,
    /// Absolute path to the skill directory in the Cache or Store.
    pub source: PathBuf,
    pub provenance: Provenance,
}

/// Every linkable skill: authored (from the manifest + Store) and vendored
/// (from the lock + Cache), keyed for name lookup.
pub fn linkable(sd: &Skilldock) -> Result<Vec<ResolvedLink>> {
    let manifest = Manifest::read(&sd.manifest_path())?;
    let lock = Lock::read(&sd.lock_path())?;
    Ok(links_of(sd, &manifest, &lock))
}

/// Build the linkable set from an already-read manifest + lock.
fn links_of(sd: &Skilldock, manifest: &Manifest, lock: &Lock) -> Vec<ResolvedLink> {
    let mut out = Vec::new();
    for name in &manifest.authored {
        out.push(ResolvedLink {
            source: sd.authored_skill_dir(name),
            name: name.clone(),
            provenance: Provenance::Authored,
        });
    }
    for repo in &lock.repos {
        out.extend(repo_links(sd, repo));
    }
    out
}

/// The links for one locked repo (all its vendored skills).
fn repo_links(sd: &Skilldock, repo: &crate::lock::LockRepo) -> Vec<ResolvedLink> {
    let clone = sd.cache_clone_dir(&repo.repo);
    repo.skills
        .iter()
        .map(|skill| ResolvedLink {
            name: skill.name.clone(),
            source: clone.join(&skill.path),
            provenance: Provenance::Vendored,
        })
        .collect()
}

/// Expand a repo identity to its links, erroring if the lock doesn't know it.
fn expand_repo(sd: &Skilldock, lock: &Lock, repo_id: &str) -> Result<Vec<ResolvedLink>> {
    let repo = lock
        .repos
        .iter()
        .find(|r| r.repo == repo_id)
        .ok_or_else(|| Error::Invalid(format!("no vendored repo '{repo_id}' in the lock")))?;
    Ok(repo_links(sd, repo))
}

/// Resolve inputs to the set of skill **names** they refer to, without
/// requiring the skill to still exist in the model — used by `unlink`, which
/// must remove a link even after its source is gone. A `/` marks a repo
/// identity (expanded via the lock); anything else is taken as a name verbatim.
pub fn resolve_names(sd: &Skilldock, inputs: &[String]) -> Result<Vec<String>> {
    let lock = Lock::read(&sd.lock_path())?;
    let mut names: Vec<String> = Vec::new();
    for input in inputs {
        if input.contains('/') {
            names.extend(expand_repo(sd, &lock, input)?.into_iter().map(|l| l.name));
        } else {
            names.push(input.clone());
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

/// Resolve inputs (skill names or repo identities) to a deduped, name-sorted
/// set of links. A `/` marks a repo identity; anything else is a skill name.
pub fn resolve_inputs(sd: &Skilldock, inputs: &[String]) -> Result<Vec<ResolvedLink>> {
    let manifest = Manifest::read(&sd.manifest_path())?;
    let lock = Lock::read(&sd.lock_path())?;
    let all = links_of(sd, &manifest, &lock);

    // Index names -> links so ambiguity (a name owned by two sources) is caught.
    let mut by_name: BTreeMap<&str, Vec<&ResolvedLink>> = BTreeMap::new();
    for link in &all {
        by_name.entry(&link.name).or_default().push(link);
    }

    let mut picked: BTreeMap<String, ResolvedLink> = BTreeMap::new();
    for input in inputs {
        if input.contains('/') {
            for link in expand_repo(sd, &lock, input)? {
                picked.insert(link.name.clone(), link);
            }
        } else {
            match by_name.get(input.as_str()).map(Vec::as_slice) {
                Some([one]) => {
                    picked.insert(input.clone(), (*one).clone());
                }
                Some(many) if many.len() > 1 => {
                    return Err(Error::Invalid(format!(
                        "'{input}' is ambiguous ({} skills share that name); link by repo identity",
                        many.len()
                    )));
                }
                _ => {
                    return Err(Error::Invalid(format!(
                        "'{input}' is neither a known skill nor a vendored repo"
                    )));
                }
            }
        }
    }

    Ok(picked.into_values().collect())
}
