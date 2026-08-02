//! Expand declared skill specs against a checked-out Source into exact,
//! hashed lock entries.
//!
//! A bare path resolves to one skill (its basename, unless renamed); a glob
//! resolves to every matching directory that is a skill (contains `SKILL.md`).
//! The lock only ever holds these concrete results — never globs.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::{Error, Result};
use crate::hash::hash_dir;
use crate::lock::LockSkill;
use crate::manifest::SkillSpec;

/// A directory is a skill iff it contains a `SKILL.md` (the rule from the
/// `CONTEXT.md` glossary).
fn is_skill_dir(dir: &Path) -> bool {
    dir.join("SKILL.md").is_file()
}

/// Expand every spec against `clone_dir`, returning concrete lock entries
/// sorted by name. Errors if an exact path isn't a skill, or a glob matches no
/// skills — either is a declaration that resolves to nothing.
pub fn expand_skills(clone_dir: &Path, specs: &[SkillSpec]) -> Result<Vec<LockSkill>> {
    // Keyed by subpath so overlapping specs/globs de-duplicate deterministically.
    let mut by_path: BTreeMap<String, LockSkill> = BTreeMap::new();

    for spec in specs {
        if spec.is_glob() {
            expand_glob(clone_dir, spec.path(), &mut by_path)?;
        } else {
            let (path, skill) = resolve_exact(clone_dir, spec.path(), spec.declared_name())?;
            by_path.insert(path, skill);
        }
    }

    let mut skills: Vec<LockSkill> = by_path.into_values().collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

fn resolve_exact(
    clone_dir: &Path,
    subpath: &str,
    rename: Option<&str>,
) -> Result<(String, LockSkill)> {
    let rel = normalize(subpath);
    let dir = clone_dir.join(&rel);
    if !is_skill_dir(&dir) {
        return Err(Error::Invalid(format!(
            "declared skill '{rel}' has no SKILL.md in the source"
        )));
    }
    let name = rename.map(str::to_string).unwrap_or_else(|| basename(&rel));
    let hash = hash_dir(&dir)?;
    Ok((
        rel.clone(),
        LockSkill {
            name,
            path: rel,
            hash,
        },
    ))
}

fn expand_glob(
    clone_dir: &Path,
    pattern: &str,
    out: &mut BTreeMap<String, LockSkill>,
) -> Result<()> {
    let rel_pattern = normalize(pattern);
    let full = clone_dir.join(&rel_pattern);
    let full = full.to_string_lossy();

    let mut matched = 0usize;
    let paths =
        glob::glob(&full).map_err(|e| Error::Invalid(format!("bad glob '{pattern}': {e}")))?;
    for entry in paths {
        let path = entry.map_err(|e| Error::io(e.path().to_path_buf(), e.into()))?;
        if !path.is_dir() || !is_skill_dir(&path) {
            continue;
        }
        let rel = path
            .strip_prefix(clone_dir)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        let hash = hash_dir(&path)?;
        out.insert(
            rel.clone(),
            LockSkill {
                name: basename(&rel),
                path: rel,
                hash,
            },
        );
        matched += 1;
    }

    if matched == 0 {
        return Err(Error::Invalid(format!(
            "glob '{pattern}' matched no skills in the source"
        )));
    }
    Ok(())
}

/// Trim surrounding/trailing slashes from a declared subpath.
fn normalize(subpath: &str) -> String {
    subpath.trim_matches('/').to_string()
}

/// The last path component — the default skill name.
fn basename(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}

#[cfg(test)]
mod tests {
    use super::expand_skills;
    use crate::manifest::SkillSpec;

    fn skill(dir: &std::path::Path, sub: &str) {
        let d = dir.join(sub);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("SKILL.md"), "---\nname: x\n---\n").unwrap();
    }

    #[test]
    fn exact_path_resolves_one_skill() {
        let tmp = tempfile::tempdir().unwrap();
        skill(tmp.path(), "skills/grilling");
        let out = expand_skills(tmp.path(), &[SkillSpec::Path("skills/grilling".into())]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "grilling");
        assert_eq!(out[0].path, "skills/grilling");
        assert!(out[0].hash.starts_with("sha256:"));
    }

    #[test]
    fn rename_table_overrides_name() {
        let tmp = tempfile::tempdir().unwrap();
        skill(tmp.path(), "skills/domain-modeling");
        let out = expand_skills(
            tmp.path(),
            &[SkillSpec::Named {
                name: "dm".into(),
                path: "skills/domain-modeling".into(),
            }],
        )
        .unwrap();
        assert_eq!(out[0].name, "dm");
        assert_eq!(out[0].path, "skills/domain-modeling");
    }

    #[test]
    fn glob_expands_to_matching_skills_only() {
        let tmp = tempfile::tempdir().unwrap();
        skill(tmp.path(), "skills/eng/grilling");
        skill(tmp.path(), "skills/eng/domain-modeling");
        std::fs::create_dir_all(tmp.path().join("skills/eng/not-a-skill")).unwrap();
        let out = expand_skills(tmp.path(), &[SkillSpec::Path("skills/eng/*".into())]).unwrap();
        let names: Vec<_> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["domain-modeling", "grilling"]);
    }

    #[test]
    fn missing_exact_path_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let err = expand_skills(tmp.path(), &[SkillSpec::Path("nope".into())]);
        assert!(err.is_err());
    }

    #[test]
    fn glob_matching_nothing_errors() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("skills")).unwrap();
        let err = expand_skills(tmp.path(), &[SkillSpec::Path("skills/*".into())]);
        assert!(err.is_err());
    }
}
