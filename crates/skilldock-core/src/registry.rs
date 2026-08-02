//! The Consumer registry (`links.txt`): absolute paths of projects holding at
//! least one Link, so `prune --all` (a later ticket) can find them.
//!
//! Machine-local and gitignored — it is downstream state, orthogonal to the
//! manifest/lock. Kept sorted and unique.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::skilldock::Skilldock;

/// Read the registered Consumer paths (empty if the registry is absent).
pub fn read(sd: &Skilldock) -> Result<Vec<PathBuf>> {
    let path = sd.links_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::io(&path, e)),
    }
}

/// Register `consumer`; returns `false` if it was already present.
pub fn add(sd: &Skilldock, consumer: &Path) -> Result<bool> {
    let canonical = canonicalize(consumer);
    let mut entries = read(sd)?;
    if entries.contains(&canonical) {
        return Ok(false);
    }
    entries.push(canonical);
    write(sd, &mut entries)?;
    Ok(true)
}

/// Deregister `consumer`; returns `false` if it was not present.
pub fn remove(sd: &Skilldock, consumer: &Path) -> Result<bool> {
    let canonical = canonicalize(consumer);
    let mut entries = read(sd)?;
    let before = entries.len();
    entries.retain(|e| *e != canonical);
    if entries.len() == before {
        return Ok(false);
    }
    write(sd, &mut entries)?;
    Ok(true)
}

fn write(sd: &Skilldock, entries: &mut Vec<PathBuf>) -> Result<()> {
    entries.sort();
    entries.dedup();
    let path = sd.links_path();
    let mut text = String::new();
    for e in entries.iter() {
        text.push_str(&e.to_string_lossy());
        text.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(&path, text).map_err(|e| Error::io(&path, e))
}

/// Canonicalize when the path exists (so `.`/symlinks normalize), else keep the
/// path as given so a removed project can still be deregistered.
fn canonicalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
