//! Consumer-level link housekeeping shared by the link/unlink/prune ops: the
//! project `.claude/skills` entry link, and tearing a project down once its
//! last skill is unlinked.

use std::path::Path;

use crate::consumer::Consumer;
use crate::error::{Error, Result};
use crate::linkfs;
use crate::registry;
use crate::skilldock::Skilldock;

/// Require that a project consumer's directory exists (global consumers always
/// pass — their trees are created on demand). Keeps this validation in core
/// (ADR-0001), consistent across every link op.
pub fn require_consumer(consumer: &Consumer) -> Result<()> {
    if let Consumer::Project(dir) = consumer {
        if !dir.is_dir() {
            return Err(Error::Invalid(format!(
                "project path does not exist: {}",
                dir.display()
            )));
        }
    }
    Ok(())
}

/// Whether the symlink at `link` points into this skilldock's Cache or Store —
/// i.e. this skilldock owns it. Used to scope global unlink/prune/relink so they
/// never touch another store's links.
pub fn owned_by_skilldock(sd: &Skilldock, link: &Path) -> bool {
    match std::fs::read_link(link) {
        Ok(target) => target.starts_with(sd.cache()) || target.starts_with(sd.store()),
        Err(_) => false,
    }
}

/// Ensure a project's `.claude/skills -> ../.agents/skills` entry link exists so
/// Claude Code discovers its linked skills. No-op for a global consumer or when
/// a non-symlink already sits there. Returns whether it created the link.
pub fn ensure_entry_link(consumer: &Consumer) -> Result<bool> {
    let (Some(entry), Some(target)) = (consumer.entry_link(), consumer.entry_link_target()) else {
        return Ok(false);
    };
    if linkfs::is_symlink(&entry) {
        return Ok(false);
    }
    if entry.exists() {
        return Ok(false); // a real dir/file — leave it untouched
    }
    if let Some(parent) = entry.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::os::unix::fs::symlink(target, &entry).map_err(|e| Error::io(&entry, e))?;
    Ok(true)
}

/// Whether a project's `.agents/skills` directory holds no entries.
fn skills_dir_empty(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut it) => it.next().is_none(),
        Err(_) => true,
    }
}

/// After a removal, if a project's `.agents/skills` is now empty, drop the entry
/// link, remove the empty dir, and deregister the project. Returns whether it
/// deregistered. No-op for a global consumer.
pub fn cleanup_project_if_empty(sd: &Skilldock, consumer: &Consumer) -> Result<bool> {
    let Consumer::Project(dir) = consumer else {
        return Ok(false);
    };
    let skills_dir = dir.join(".agents/skills");
    if !skills_dir.is_dir() || !skills_dir_empty(&skills_dir) {
        return Ok(false);
    }

    if let Some(entry) = consumer.entry_link() {
        if linkfs::is_symlink(&entry) {
            std::fs::remove_file(&entry).map_err(|e| Error::io(&entry, e))?;
        }
    }
    std::fs::remove_dir(&skills_dir).map_err(|e| Error::io(&skills_dir, e))?;
    registry::remove(sd, dir)
}
