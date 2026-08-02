use std::path::Path;

use crate::error::{Error, Result};
use crate::registry;
use crate::skilldock::Skilldock;

/// Register a project consumer in `links.txt` explicitly. Errors if the path
/// does not exist. Returns whether it was newly added.
pub fn register(sd: &Skilldock, consumer: &Path) -> Result<bool> {
    if !consumer.is_dir() {
        return Err(Error::Invalid(format!(
            "project path does not exist: {}",
            consumer.display()
        )));
    }
    registry::add(sd, consumer)
}

/// Deregister a project consumer from `links.txt` explicitly. Works whether or
/// not the path still exists. Returns whether it was present.
pub fn deregister(sd: &Skilldock, consumer: &Path) -> Result<bool> {
    registry::remove(sd, consumer)
}
