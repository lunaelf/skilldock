//! Per-skill content hashing — the integrity half of a lock entry.
//!
//! A skill's `sha256:` digest lets `doctor --verify` (a later ticket) detect
//! Cache tampering or an upstream force-push under a pinned SHA.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// Compute a deterministic content hash for a skill directory.
///
/// The hash covers every regular file under `dir`, keyed by its relative path,
/// so it is stable across machines and independent of filesystem walk order.
/// Returned as `sha256:<hex>` for [`crate::lock::LockSkill::hash`] — the
/// per-skill integrity field.
pub fn hash_dir(dir: &Path) -> Result<String> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    collect(dir, dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel, bytes) in &files {
        // Length-prefix each field so path/content boundaries can't be forged.
        hasher.update((rel.len() as u64).to_le_bytes());
        hasher.update(rel.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(dir, e))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| Error::io(&path, e))?;
        // Skip .git so a clone's history never changes a skill's content hash.
        if path.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        if file_type.is_dir() {
            collect(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = path.strip_prefix(root).map_err(|_| {
                Error::Invalid(format!(
                    "path {} escaped {}",
                    path.display(),
                    root.display()
                ))
            })?;
            let rel = rel.to_string_lossy().replace('\\', "/");
            let bytes = std::fs::read(&path).map_err(|e| Error::io(&path, e))?;
            out.push((rel, bytes));
        }
        // Symlinks and other special files are ignored for hashing purposes.
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::hash_dir;

    fn write(dir: &std::path::Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn is_deterministic_and_prefixed() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "SKILL.md", "hello");
        write(tmp.path(), "sub/a.txt", "world");
        let h1 = hash_dir(tmp.path()).unwrap();
        let h2 = hash_dir(tmp.path()).unwrap();
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256:"));
    }

    #[test]
    fn changes_with_content() {
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "SKILL.md", "one");
        let b = tempfile::tempdir().unwrap();
        write(b.path(), "SKILL.md", "two");
        assert_ne!(hash_dir(a.path()).unwrap(), hash_dir(b.path()).unwrap());
    }

    #[test]
    fn ignores_dot_git() {
        let a = tempfile::tempdir().unwrap();
        write(a.path(), "SKILL.md", "same");
        let h_before = hash_dir(a.path()).unwrap();
        write(a.path(), ".git/HEAD", "ref: refs/heads/main");
        assert_eq!(h_before, hash_dir(a.path()).unwrap());
    }
}
