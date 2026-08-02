use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::skilldock::Skilldock;

/// What `author` did, so adapters can report it precisely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorOutcome {
    pub name: String,
    /// A fresh `SKILL.md` scaffold was written (the skill did not exist).
    pub scaffolded: bool,
    /// The name was already in the `authored` list before this call.
    pub already_listed: bool,
}

/// Mark or scaffold an authored skill.
///
/// If the skill's Store directory does not already contain a `SKILL.md`, a
/// minimal scaffold is written. Either way the name is recorded in the
/// manifest's `authored` list so `doctor` won't flag it as an orphan.
/// Idempotent: re-running never overwrites an existing skill or duplicates the
/// list entry.
pub fn author(sd: &Skilldock, name: &str) -> Result<AuthorOutcome> {
    validate_name(name)?;

    let dir = sd.authored_skill_dir(name);
    let skill_md = dir.join("SKILL.md");

    let scaffolded = if skill_md.exists() {
        false
    } else {
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        std::fs::write(&skill_md, scaffold(name)).map_err(|e| Error::io(&skill_md, e))?;
        true
    };

    let mut manifest = Manifest::read(&sd.manifest_path())?;
    let added = manifest.add_authored(name);
    if added {
        manifest.write(&sd.manifest_path())?;
    }

    Ok(AuthorOutcome {
        name: name.to_string(),
        scaffolded,
        already_listed: !added,
    })
}

/// A skill name must be a single clean path component.
fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Invalid("skill name must not be empty".into()));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(Error::Invalid(format!(
            "skill name '{name}' must be a single path component"
        )));
    }
    if name == "." || name == ".." {
        return Err(Error::Invalid(format!(
            "'{name}' is not a valid skill name"
        )));
    }
    if name.chars().any(char::is_whitespace) {
        return Err(Error::Invalid(format!(
            "skill name '{name}' must not contain whitespace"
        )));
    }
    Ok(())
}

/// A minimal valid `SKILL.md` for a new authored skill.
fn scaffold(name: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: TODO — one line on when to use this skill.\n---\n\n# {name}\n\nTODO: write the skill instructions.\n"
    )
}
