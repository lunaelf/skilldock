//! Per-Consumer link status — the read that backs the GUI's per-skill
//! Link/Unlink toggle. Deliberately separate from `list` (which is
//! provenance-grouped inventory, independent of any Consumer).

use std::collections::BTreeSet;

use serde::Serialize;

use crate::consumer::Consumer;
use crate::error::Result;
use crate::linkfs;
use crate::resolve;
use crate::skilldock::Skilldock;

/// Whether a dock skill is linked into a Consumer, and if so whether the link
/// resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum LinkState {
    /// No link for this skill in the Consumer.
    Unlinked,
    /// Linked and the symlink resolves (not broken). A link that resolves to a
    /// stale path still reads as `Linked` — re-pointing it is relink's job.
    Linked,
    /// Linked but the symlink is broken (its Source is gone).
    Dangling,
}

/// One dock skill's link state in a given Consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SkillLinkStatus {
    pub name: String,
    pub state: LinkState,
}

/// For each dock skill (authored + vendored), its [`LinkState`] in `consumer`.
///
/// Names are unique and sorted. A name owned by two sources (a collision doctor
/// flags) collapses to one row, since a link lives at
/// `<consumer>/.agents/skills/<name>`, keyed only by name.
pub fn link_status(sd: &Skilldock, consumer: &Consumer) -> Result<Vec<SkillLinkStatus>> {
    let names: BTreeSet<String> = resolve::linkable(sd)?.into_iter().map(|l| l.name).collect();
    Ok(names
        .into_iter()
        .map(|name| SkillLinkStatus {
            state: state_of(consumer, &name),
            name,
        })
        .collect())
}

/// A skill is linked if any of its Consumer destinations is a symlink; dangling
/// if such a symlink is broken. (A project has one destination, global two.)
fn state_of(consumer: &Consumer, name: &str) -> LinkState {
    let mut linked = false;
    let mut dangling = false;
    for dest in consumer.link_dests(name) {
        if linkfs::is_symlink(&dest) {
            linked = true;
            if linkfs::is_broken_symlink(&dest) {
                dangling = true;
            }
        }
    }
    match (linked, dangling) {
        (false, _) => LinkState::Unlinked,
        (true, true) => LinkState::Dangling,
        (true, false) => LinkState::Linked,
    }
}
