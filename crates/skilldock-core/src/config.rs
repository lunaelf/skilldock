//! The dock config (`~/.skilldock/config.toml`): the data-repo remote and
//! user preferences, written by `init` and read on a fresh-machine bootstrap.
//! Absent config resolves to defaults.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::tomlio;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Git remote of the data repo, recorded at `init` for later `sync`/restore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_repo: Option<String>,
}

impl Config {
    /// Read the config at `path`, returning defaults if it is absent.
    pub fn read(path: &Path) -> Result<Self> {
        tomlio::read_or_default(path)
    }

    /// Write the config to `path`, creating parent directories as needed.
    pub fn write(&self, path: &Path) -> Result<()> {
        tomlio::write(self, path)
    }
}
