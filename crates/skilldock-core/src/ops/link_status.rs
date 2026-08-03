//! Per-Consumer link status — the read that backs the GUI's per-skill
//! Link/Unlink toggle. Deliberately separate from `list` (which is
//! provenance-grouped inventory, independent of any Consumer).

use std::collections::BTreeSet;

use serde::Serialize;

use crate::consumer::Consumer;
use crate::error::Result;
use crate::linkfs;
use crate::linking;
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
            state: state_of(sd, consumer, &name),
            name,
        })
        .collect())
}

/// A skill is linked if any of its Consumer destinations is a symlink; dangling
/// if such a symlink is broken. (A project has one destination, global two.)
///
/// For a global Consumer, a link this skilldock does not own (its target lies
/// outside this dock's Cache/Store) is ignored — exactly as `unlink`/`prune`/
/// `relink` skip it — so the toggle never reports a state the link-family ops
/// would refuse to act on. Such a link reads as `Unlinked`, and `link` (which is
/// not ownership-scoped) can reclaim it via a `--force` replace.
fn state_of(sd: &Skilldock, consumer: &Consumer, name: &str) -> LinkState {
    let global = matches!(consumer, Consumer::Global { .. });
    let mut linked = false;
    let mut dangling = false;
    for dest in consumer.link_dests(name) {
        if !linkfs::is_symlink(&dest) {
            continue;
        }
        if global && !linking::owned_by_skilldock(sd, &dest) {
            continue;
        }
        linked = true;
        if linkfs::is_broken_symlink(&dest) {
            dangling = true;
        }
    }
    match (linked, dangling) {
        (false, _) => LinkState::Unlinked,
        (true, true) => LinkState::Dangling,
        (true, false) => LinkState::Linked,
    }
}
