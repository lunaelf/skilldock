use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::glob::is_glob;
use crate::tomlio;

/// The declared manifest (`skilldock.toml`), hand-editable.
///
/// This is the *declared* half of the cargo-style split: what the user asked
/// for. The resolved half is [`crate::lock::Lock`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Authored skill names tracked in the Store.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authored: Vec<String>,
    /// Declared vendored source repos.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendored: Vec<VendoredRepo>,
}

/// One declared vendored source repo and the skills wanted from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VendoredRepo {
    /// Repo identity, e.g. `github.com/mattpocock/skills`.
    pub repo: String,
    /// Branch or tag to track; omitted means the default branch.
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    /// Declared skills: bare paths, globs, or `{name, path}` renames.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillSpec>,
}

/// A declared skill entry inside a [`VendoredRepo`].
///
/// TOML permits either a bare string (a subpath, possibly a glob) or an inline
/// table `{ name, path }` for a rename. The lock never holds a [`SkillSpec`];
/// it expands globs to exact [`crate::lock::LockSkill`] entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SkillSpec {
    /// A bare subpath. May be a glob (`skills/engineering/*`). The linked name
    /// defaults to the path's basename.
    Path(String),
    /// A renamed skill: link it under `name` regardless of its subpath basename.
    Named { name: String, path: String },
}

impl SkillSpec {
    /// The declared subpath (or glob) within the repo.
    pub fn path(&self) -> &str {
        match self {
            SkillSpec::Path(p) => p,
            SkillSpec::Named { path, .. } => path,
        }
    }

    /// The explicit rename, if any. `None` means "default to the basename".
    pub fn declared_name(&self) -> Option<&str> {
        match self {
            SkillSpec::Path(_) => None,
            SkillSpec::Named { name, .. } => Some(name),
        }
    }

    /// Whether this entry is a glob (only legal in the declared manifest).
    pub fn is_glob(&self) -> bool {
        is_glob(self.path())
    }
}

impl Manifest {
    /// Parse a manifest from TOML text.
    pub fn parse(text: &str, path: &Path) -> Result<Self> {
        tomlio::parse(text, path)
    }

    /// Serialize to TOML text.
    pub fn to_toml(&self) -> Result<String> {
        tomlio::to_string(self, Path::new("skilldock.toml"))
    }

    /// Read the manifest at `path`, returning an empty manifest if it is absent.
    pub fn read(path: &Path) -> Result<Self> {
        tomlio::read_or_default(path)
    }

    /// Write the manifest to `path`, creating parent directories as needed.
    pub fn write(&self, path: &Path) -> Result<()> {
        tomlio::write(self, path)
    }

    /// Add an authored skill name, keeping the list sorted and unique.
    /// Returns `false` if it was already present.
    pub fn add_authored(&mut self, name: &str) -> bool {
        if self.authored.iter().any(|n| n == name) {
            return false;
        }
        self.authored.push(name.to_string());
        self.authored.sort();
        true
    }

    /// Declare a vendored repo: merge into an existing `[[vendored]]` entry for
    /// `repo` (updating its ref and unioning skills) or append a new one.
    pub fn declare_vendored(&mut self, repo: &str, git_ref: Option<String>, skills: &[SkillSpec]) {
        match self.vendored.iter_mut().find(|v| v.repo == repo) {
            Some(existing) => {
                if git_ref.is_some() {
                    existing.git_ref = git_ref;
                }
                for spec in skills {
                    if !existing.skills.contains(spec) {
                        existing.skills.push(spec.clone());
                    }
                }
            }
            None => self.vendored.push(VendoredRepo {
                repo: repo.to_string(),
                git_ref,
                skills: skills.to_vec(),
            }),
        }
    }
}
