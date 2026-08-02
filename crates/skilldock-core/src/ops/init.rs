use std::path::PathBuf;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::git;
use crate::ops::sync::{sync, SyncOutcome};
use crate::skilldock::Skilldock;

/// The canonical data-repo pre-commit gate `init` installs into the Store:
/// block a commit while `doctor` finds errors. Held as a string constant (not a
/// repo file) because a `cargo install`ed binary has no source tree to read.
const PRE_COMMIT_HOOK: &str = "#!/usr/bin/env sh\n\
# skilldock data-repo pre-commit gate: blocks a commit while the dock is\n\
# inconsistent (doctor exits non-zero on errors). Bypass with --no-verify.\n\
exec skilldock doctor\n";

/// What `init` produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOutcome {
    /// The Store checkout path.
    pub store: PathBuf,
    /// The result of the follow-up `sync`.
    pub synced: SyncOutcome,
}

/// Fresh-machine bootstrap: clone the data repo into `~/.skilldock/store`, record
/// its remote in `config.toml`, install the pre-commit gate, and `sync` to
/// populate the Cache from the lock.
pub fn init(sd: &Skilldock, data_repo_url: &str) -> Result<InitOutcome> {
    let store = sd.store();
    if store.join(".git").is_dir() {
        return Err(Error::Invalid(format!(
            "already initialized: {} is a git checkout — run `skilldock sync`, or remove it to re-init",
            store.display()
        )));
    }
    if store.exists() {
        return Err(Error::Invalid(format!(
            "{} already exists but is not a Skilldock checkout; remove it first",
            store.display()
        )));
    }

    // git clone needs an empty/absent target; ensure the Skilldock root exists first.
    std::fs::create_dir_all(sd.root()).map_err(|e| Error::io(sd.root(), e))?;
    git::clone(data_repo_url, &store)?;

    Config {
        data_repo: Some(data_repo_url.to_string()),
    }
    .write(&sd.config_path())?;

    install_pre_commit_hook(sd)?;

    let synced = sync(sd)?;
    Ok(InitOutcome { store, synced })
}

/// Write the pre-commit gate into the freshly cloned Store's `.git/hooks`.
fn install_pre_commit_hook(sd: &Skilldock) -> Result<()> {
    let hooks_dir = sd.store().join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).map_err(|e| Error::io(&hooks_dir, e))?;
    let hook = hooks_dir.join("pre-commit");
    std::fs::write(&hook, PRE_COMMIT_HOOK).map_err(|e| Error::io(&hook, e))?;

    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&hook, perms).map_err(|e| Error::io(&hook, e))?;
    Ok(())
}
