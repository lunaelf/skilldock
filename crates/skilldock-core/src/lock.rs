use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::glob::is_glob;
use crate::tomlio;

/// The resolved lock (`skilldock.lock`), tool-owned.
///
/// The *resolved* half of the cargo-style split: every glob in the manifest is
/// expanded here to exact, hashed entries pinned to a per-repo SHA.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    /// One entry per vendored source repo.
    #[serde(rename = "repo", default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<LockRepo>,
}

/// A vendored source repo pinned to one commit, with its expanded skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockRepo {
    /// Repo identity, e.g. `github.com/mattpocock/skills`.
    pub repo: String,
    /// The URL `git clone` fetches from. Recorded so `sync` reproduces the Cache
    /// from the lock alone, including non-derivable (SSH/mirror/local) sources.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// The exact commit SHA every skill from this repo is pinned to.
    pub resolved: String,
    /// Exact, hashed skills resolved from this repo (never globs).
    #[serde(rename = "skill", default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<LockSkill>,
}

impl LockRepo {
    /// The URL to clone from: the recorded one, or derived from the identity
    /// when a hand-edited/older lock omitted it.
    pub fn clone_url(&self) -> String {
        if self.url.is_empty() {
            crate::source::clone_url_for(&self.repo)
        } else {
            self.url.clone()
        }
    }
}

/// One resolved skill: exact subpath, linked name, and content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockSkill {
    /// The name this skill links as.
    pub name: String,
    /// The exact subpath within the repo (never a glob).
    pub path: String,
    /// Content integrity hash, e.g. `sha256:...`.
    pub hash: String,
}

impl Lock {
    /// Parse a lock from TOML text.
    pub fn parse(text: &str, path: &Path) -> Result<Self> {
        tomlio::parse(text, path)
    }

    /// Serialize to TOML text, rejecting any glob that slipped into a path.
    pub fn to_toml(&self) -> Result<String> {
        self.validate()?;
        tomlio::to_string(self, Path::new("skilldock.lock"))
    }

    /// Read the lock at `path`, returning an empty lock if it is absent.
    pub fn read(path: &Path) -> Result<Self> {
        tomlio::read_or_default(path)
    }

    /// Write the lock to `path`, creating parent directories as needed.
    /// The globs-never-in-lock invariant is enforced before any bytes are written.
    pub fn write(&self, path: &Path) -> Result<()> {
        self.validate()?;
        tomlio::write(self, path)
    }

    /// Insert `repo`, replacing any existing entry with the same identity
    /// (a re-resolve), keeping repos sorted by identity for stable output.
    pub fn upsert_repo(&mut self, repo: LockRepo) {
        self.repos.retain(|r| r.repo != repo.repo);
        self.repos.push(repo);
        self.repos.sort_by(|a, b| a.repo.cmp(&b.repo));
    }

    /// Mutable access to a locked repo by identity.
    pub fn repo_mut(&mut self, repo: &str) -> Option<&mut LockRepo> {
        self.repos.iter_mut().find(|r| r.repo == repo)
    }

    /// Drop a locked repo entirely; returns whether it was present.
    pub fn remove_repo(&mut self, repo: &str) -> bool {
        let before = self.repos.len();
        self.repos.retain(|r| r.repo != repo);
        self.repos.len() < before
    }

    /// Find the identity of the repo that provides a resolved skill named
    /// `name`. Returns an error if two repos share that name (ambiguous), and
    /// `None` if no vendored skill has it.
    pub fn repo_of_skill(&self, name: &str) -> Result<Option<String>> {
        let mut owners: Vec<&str> = self
            .repos
            .iter()
            .filter(|r| r.skills.iter().any(|s| s.name == name))
            .map(|r| r.repo.as_str())
            .collect();
        owners.sort();
        match owners.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some((*one).to_string())),
            many => Err(Error::Invalid(format!(
                "'{name}' is provided by {} repos ({}); remove by repo identity",
                many.len(),
                many.join(", ")
            ))),
        }
    }

    /// The invariant that separates lock from manifest: no path may be a glob.
    pub fn validate(&self) -> Result<()> {
        for repo in &self.repos {
            for skill in &repo.skills {
                if is_glob(&skill.path) {
                    return Err(Error::GlobInLock(skill.path.clone()));
                }
            }
        }
        Ok(())
    }
}
