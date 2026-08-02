use crate::consumer::Consumer;
use crate::error::{Error, Result};
use crate::linkfs::{self, LinkStatus};
use crate::linking;
use crate::registry;
use crate::resolve;
use crate::skilldock::Skilldock;

/// What `link` did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkOutcome {
    /// Skills newly linked (or re-pointed with `force`).
    pub linked: Vec<String>,
    /// Skills already linked to the same Source.
    pub already: Vec<String>,
}

/// Link the named skills (or every skill of a repo) into `consumer`, symlinking
/// each from its Source — the Cache for vendored, the Store for authored — and
/// registering a project consumer in `links.txt`.
pub fn link(
    sd: &Skilldock,
    consumer: &Consumer,
    inputs: &[String],
    force: bool,
) -> Result<LinkOutcome> {
    linking::require_consumer(consumer)?;
    let skills = resolve::resolve_inputs(sd, inputs)?;
    let mut outcome = LinkOutcome::default();

    for skill in &skills {
        if !skill.source.is_dir() {
            return Err(Error::Invalid(format!(
                "source for '{}' is missing: {}",
                skill.name,
                skill.source.display()
            )));
        }
        let mut newly = false;
        for dest in consumer.link_dests(&skill.name) {
            match linkfs::make_link(&dest, &skill.source, force)? {
                LinkStatus::Created | LinkStatus::Replaced => newly = true,
                LinkStatus::Exists => {}
            }
        }
        if newly {
            outcome.linked.push(skill.name.clone());
        } else {
            outcome.already.push(skill.name.clone());
        }
    }

    // Project consumers get the Claude entry link and a registry entry.
    if !skills.is_empty() {
        linking::ensure_entry_link(consumer)?;
        if let Some(dir) = consumer.registry_path() {
            registry::add(sd, dir)?;
        }
    }

    Ok(outcome)
}
