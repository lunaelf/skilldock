//! Shared TOML file I/O for the manifest and lock: read-or-default and
//! atomic-enough write, with path-tagged errors. Keeps the parse/serialize
//! plumbing in one place instead of repeated per type.

use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{Error, Result};

/// Parse TOML text into `T`, tagging failures with `path` for diagnostics.
pub fn parse<T: DeserializeOwned>(text: &str, path: &Path) -> Result<T> {
    toml::from_str(text).map_err(|source| Error::TomlParse {
        path: path.to_path_buf(),
        source,
    })
}

/// Read and parse a TOML file, returning `T::default()` when the file is absent.
pub fn read_or_default<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text, path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// Serialize `value` to pretty TOML text, tagging failures with `path`.
pub fn to_string<T: Serialize>(value: &T, path: &Path) -> Result<String> {
    toml::to_string_pretty(value).map_err(|source| Error::TomlWrite {
        path: path.to_path_buf(),
        source,
    })
}

/// Serialize `value` and write it to `path`, creating parent directories.
pub fn write<T: Serialize>(value: &T, path: &Path) -> Result<()> {
    let text = to_string(value, path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(path, text).map_err(|e| Error::io(path, e))
}
