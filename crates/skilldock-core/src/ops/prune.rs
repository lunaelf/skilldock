use std::path::PathBuf;

use serde::Serialize;

use crate::consumer::Consumer;
use crate::error::{Error, Result};
use crate::linkfs;
use crate::linking;
use crate::registry;
use crate::skilldock::Skilldock;

/// What `prune` did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PruneOutcome {
    /// Dangling link names that were removed.
    pub pruned: Vec<String>,
    /// Whether a project was deregistered (pruning emptied it).
    pub deregistered: bool,
}

/// Remove dangling (broken) links from `consumer`. For a global consumer only
/// broken links pointing into this skilldock are removed; other stores' links
/// are left alone.
pub fn prune(sd: &Skilldock, consumer: &Consumer) -> Result<PruneOutcome> {
    linking::require_consumer(consumer)?;
    let global = matches!(consumer, Consumer::Global { .. });
    let mut outcome = PruneOutcome::default();

    for dir in consumer.skills_dirs() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(Error::io(&dir, e)),
        };
        for entry in entries {
            let entry = entry.map_err(|e| Error::io(&dir, e))?;
            let path = entry.path();
            if !linkfs::is_broken_symlink(&path) {
                continue;
            }
            if global && !linking::owned_by_skilldock(sd, &path) {
                continue;
            }
            std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))?;
            outcome
                .pruned
                .push(entry.file_name().to_string_lossy().into_owned());
        }
    }

    outcome.pruned.sort();
    outcome.pruned.dedup();
    outcome.deregistered = linking::cleanup_project_if_empty(sd, consumer)?;
    Ok(outcome)
}

/// Prune dangling links from every registered project Consumer — the
/// cross-`links.txt` batch form of [`prune`], used by `doctor --fix`. A project
/// emptied by pruning is deregistered (via [`prune`]); registered projects whose
/// directory is gone are skipped (doctor reports those separately).
pub fn prune_all(sd: &Skilldock) -> Result<Vec<(PathBuf, PruneOutcome)>> {
    let mut out = Vec::new();
    // `registry::read` snapshots the list, so deregistering emptied projects
    // mid-iteration is safe.
    for dir in registry::read(sd)? {
        if dir.is_dir() {
            let outcome = prune(sd, &Consumer::Project(dir.clone()))?;
            out.push((dir, outcome));
        }
    }
    Ok(out)
}
