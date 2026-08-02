//! A **Consumer**: a place that receives skills as links pointing at their
//! Source (CONTEXT.md).
//!
//! Either a single project (links live under `<dir>/.agents/skills/`, with a
//! `.claude/skills` entry link so Claude Code sees them) or the global config
//! (`~/.agents` and `~/.claude`, double-written). Global roots are held
//! explicitly rather than resolved from `$HOME`, so global linking is testable
//! against temp directories without mutating the environment.

use std::path::{Path, PathBuf};

/// Where linked skills are installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consumer {
    /// A project: links under `<dir>/.agents/skills/`.
    Project(PathBuf),
    /// The global config: skills double-written into both trees.
    Global { agents: PathBuf, claude: PathBuf },
}

impl Consumer {
    /// A project consumer at `dir`.
    pub fn project(dir: impl Into<PathBuf>) -> Self {
        Consumer::Project(dir.into())
    }

    /// The directories that hold per-skill links (one for a project, two global).
    pub fn skills_dirs(&self) -> Vec<PathBuf> {
        match self {
            Consumer::Project(dir) => vec![dir.join(".agents/skills")],
            Consumer::Global { agents, claude } => {
                vec![agents.join("skills"), claude.join("skills")]
            }
        }
    }

    /// The link destinations for a skill named `name` (one for a project, two
    /// global — both point at the Source).
    pub fn link_dests(&self, name: &str) -> Vec<PathBuf> {
        self.skills_dirs()
            .into_iter()
            .map(|d| d.join(name))
            .collect()
    }

    /// The project's `.claude/skills` entry link path, if this is a project.
    pub fn entry_link(&self) -> Option<PathBuf> {
        match self {
            Consumer::Project(dir) => Some(dir.join(".claude/skills")),
            Consumer::Global { .. } => None,
        }
    }

    /// The relative target the entry link points at (`.claude/skills` ->
    /// `../.agents/skills`), kept beside the paths it must stay consistent with.
    pub fn entry_link_target(&self) -> Option<&'static str> {
        match self {
            Consumer::Project(_) => Some("../.agents/skills"),
            Consumer::Global { .. } => None,
        }
    }

    /// The registry key for a project consumer (its directory), else `None`
    /// (global consumers are not registered).
    pub fn registry_path(&self) -> Option<&Path> {
        match self {
            Consumer::Project(dir) => Some(dir),
            Consumer::Global { .. } => None,
        }
    }
}
