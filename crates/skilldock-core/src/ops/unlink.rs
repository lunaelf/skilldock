use crate::consumer::Consumer;
use crate::error::{Error, Result};
use crate::linkfs;
use crate::linking;
use crate::resolve;
use crate::skilldock::Skilldock;

/// What `unlink` did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnlinkOutcome {
    /// Links that were removed.
    pub removed: Vec<String>,
    /// Names that had no link in this consumer.
    pub missing: Vec<String>,
    /// Whether the project was deregistered (its last link went away).
    pub deregistered: bool,
}

/// Remove the named skills' links from `consumer` (the inverse of `link`),
/// whether or not they still resolve. For a global consumer only links pointing
/// into this skilldock are touched. Empties a project's registration.
pub fn unlink(sd: &Skilldock, consumer: &Consumer, inputs: &[String]) -> Result<UnlinkOutcome> {
    linking::require_consumer(consumer)?;
    let names = resolve::resolve_names(sd, inputs)?;
    let global = matches!(consumer, Consumer::Global { .. });
    let mut outcome = UnlinkOutcome::default();

    for name in &names {
        let mut removed_any = false;
        for dest in consumer.link_dests(name) {
            if !linkfs::is_symlink(&dest) {
                continue;
            }
            // Global: only remove links owned by this skilldock (Cache/Store).
            if global && !linking::owned_by_skilldock(sd, &dest) {
                continue;
            }
            std::fs::remove_file(&dest).map_err(|e| Error::io(&dest, e))?;
            removed_any = true;
        }
        if removed_any {
            outcome.removed.push(name.clone());
        } else {
            outcome.missing.push(name.clone());
        }
    }

    outcome.deregistered = linking::cleanup_project_if_empty(sd, consumer)?;
    Ok(outcome)
}
