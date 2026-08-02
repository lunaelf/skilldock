use std::collections::HashMap;

use serde::Serialize;

use crate::consumer::Consumer;
use crate::error::{Error, Result};
use crate::linkfs::{self, LinkStatus};
use crate::linking;
use crate::resolve;
use crate::skilldock::Skilldock;

/// What `relink` did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RelinkOutcome {
    /// Links re-pointed to a new Source path.
    pub repointed: Vec<String>,
    /// Links already pointing at the current Source.
    pub unchanged: Vec<String>,
}

/// Re-point a consumer's existing links to the current model's Source paths.
///
/// For every symlink already present, if the model still knows that skill, its
/// link is (re)set to the current Source. Links the model no longer knows are
/// left for `prune`; missing links are not added.
pub fn relink(sd: &Skilldock, consumer: &Consumer) -> Result<RelinkOutcome> {
    linking::require_consumer(consumer)?;
    let global = matches!(consumer, Consumer::Global { .. });
    let model: HashMap<String, _> = resolve::linkable(sd)?
        .into_iter()
        .map(|l| (l.name.clone(), l))
        .collect();

    let mut outcome = RelinkOutcome::default();

    for dir in consumer.skills_dirs() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(Error::io(&dir, e)),
        };
        for entry in entries {
            let entry = entry.map_err(|e| Error::io(&dir, e))?;
            let path = entry.path();
            if !linkfs::is_symlink(&path) {
                continue;
            }
            // Global: never repoint a link owned by a different store.
            if global && !linking::owned_by_skilldock(sd, &path) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(link) = model.get(&name) else {
                continue; // unknown to the model — prune's job, not relink's
            };
            match linkfs::make_link(&path, &link.source, true)? {
                LinkStatus::Replaced | LinkStatus::Created => outcome.repointed.push(name),
                LinkStatus::Exists => outcome.unchanged.push(name),
            }
        }
    }

    outcome.repointed.sort();
    outcome.unchanged.sort();
    Ok(outcome)
}
