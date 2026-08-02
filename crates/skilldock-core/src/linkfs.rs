//! Symlink primitives for Consumer links. Unix-only (ADR-0002: symlinks used
//! unconditionally); every Link the tool creates goes through [`make_link`].

use std::path::Path;

use crate::error::{Error, Result};

/// What [`make_link`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    /// A new symlink was created.
    Created,
    /// The symlink already pointed at `target`; nothing changed.
    Exists,
    /// An existing symlink pointing elsewhere was replaced (needs `force`).
    Replaced,
}

/// Create `dest` -> `target` idempotently, creating parent dirs.
///
/// A matching symlink is left as-is; a symlink pointing elsewhere is replaced
/// only with `force`; a real file/dir in the way is never clobbered.
pub fn make_link(dest: &Path, target: &Path, force: bool) -> Result<LinkStatus> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }

    match std::fs::symlink_metadata(dest) {
        Ok(meta) if meta.file_type().is_symlink() => {
            let current = std::fs::read_link(dest).map_err(|e| Error::io(dest, e))?;
            if current == target {
                return Ok(LinkStatus::Exists);
            }
            if !force {
                return Err(Error::Invalid(format!(
                    "{} already links to {} (use --force to replace)",
                    dest.display(),
                    current.display()
                )));
            }
            std::fs::remove_file(dest).map_err(|e| Error::io(dest, e))?;
            symlink(target, dest)?;
            Ok(LinkStatus::Replaced)
        }
        Ok(_) => Err(Error::Invalid(format!(
            "{} exists and is not a symlink; refusing to clobber",
            dest.display()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            symlink(target, dest)?;
            Ok(LinkStatus::Created)
        }
        Err(e) => Err(Error::io(dest, e)),
    }
}

/// Whether `path` is a symlink whose target does not resolve (a dangling link).
pub fn is_broken_symlink(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => meta.file_type().is_symlink() && !path.exists(),
        Err(_) => false,
    }
}

/// Whether `path` is a symlink (resolving or not).
pub fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

fn symlink(target: &Path, dest: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, dest).map_err(|e| Error::io(dest, e))
}
