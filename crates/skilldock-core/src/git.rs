//! Thin shell-out wrappers over the `git` CLI (ADR-0002: no `git2`/libgit2).
//!
//! Every vendored-fetch git operation the tool needs — clone, fetch, checkout,
//! rev-parse — routes through [`run`], which turns a non-zero exit into a
//! typed [`Error::Git`] carrying stderr.

use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};

/// Run `git <args>` (optionally inside `cwd`) and return trimmed stdout.
fn run(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd.output().map_err(|e| Error::Git {
        command: format!("git {}", args.join(" ")),
        stderr: format!("could not spawn git: {e}"),
    })?;
    if !output.status.success() {
        return Err(Error::Git {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Clone `url` into `dest` (a full clone, so any pinned SHA is checkout-able).
/// The parent of `dest` must already exist.
pub fn clone(url: &str, dest: &Path) -> Result<()> {
    run(&["clone", "--quiet", url, &dest.to_string_lossy()], None).map(|_| ())
}

/// Initialize a git repository at `dir` (the directory must already exist).
/// Idempotent — `git init` on an existing repo is harmless.
pub fn init(dir: &Path) -> Result<()> {
    run(&["init", "--quiet", &dir.to_string_lossy()], None).map(|_| ())
}

/// The `git status --porcelain` output of the working tree at `dir` (empty when
/// the tree is clean). Used by `migrate` to refuse a dirty tree.
pub fn status_porcelain(dir: &Path) -> Result<String> {
    run(&["status", "--porcelain"], Some(dir))
}

/// Fetch all refs into an existing clone at `dir`.
pub fn fetch(dir: &Path) -> Result<()> {
    run(&["fetch", "--quiet", "--all", "--tags"], Some(dir)).map(|_| ())
}

/// Check out `rev` (a branch, tag, or SHA) in the clone at `dir`.
pub fn checkout(dir: &Path, rev: &str) -> Result<()> {
    run(&["checkout", "--quiet", rev], Some(dir)).map(|_| ())
}

/// Resolve `rev` to a full commit SHA in the clone at `dir`.
pub fn rev_parse(dir: &Path, rev: &str) -> Result<String> {
    run(&["rev-parse", rev], Some(dir))
}
