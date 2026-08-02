//! `doctor` — the integrity check over the whole dock.
//!
//! Cross-checks toml ↔ lock ↔ Cache ↔ Store ↔ Consumer links and reports typed
//! [`Finding`]s. Report-only by default (read-only; glob-drift and hash checks
//! only run when the clone is present at the locked SHA, never checking out).
//! Errors gate the data repo's pre-commit; `--fix` chains sync/relink/prune.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use crate::consumer::Consumer;
use crate::error::{Error, Result};
use crate::expand::expand_skills;
use crate::hash::hash_dir;
use crate::lock::Lock;
use crate::manifest::Manifest;
use crate::skilldock::Skilldock;
use crate::{git, linkfs, ops, registry, resolve};

/// Whether a finding blocks (errors gate commits) or merely informs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// The kind of inconsistency found; its [`Severity`] is fixed by the PRD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum FindingKind {
    /// A repo/skill declared in toml but absent from the lock.
    DeclaredButUnlocked,
    /// A repo in the lock but not declared in toml.
    StaleLock,
    /// The declaration re-expands to a different skill set than the lock.
    GlobDrift,
    /// A locked repo has no Cache clone.
    MissingClone,
    /// The Cache clone is checked out at a different commit than the lock.
    CacheShaMismatch,
    /// A skill's content hash differs from the lock (`--verify`).
    HashMismatch,
    /// A Cache clone not referenced by the lock.
    OrphanClone,
    /// An authored skill declared but missing in the Store.
    MissingAuthoredDir,
    /// A skill dir in the Store not listed in `authored`.
    StoreOrphan,
    /// A skill name provided by more than one source.
    NameCollision,
    /// A broken Consumer link.
    DanglingLink,
    /// A Consumer link to a skill that is no longer linkable (gone from the
    /// Store and lock).
    RemovedSkillLink,
    /// A registered Consumer whose directory is gone.
    MissingConsumerDir,
    /// A registered Consumer with no links.
    EmptyRegistration,
}

impl FindingKind {
    pub fn severity(self) -> Severity {
        use FindingKind::*;
        match self {
            DeclaredButUnlocked | StaleLock | GlobDrift | CacheShaMismatch | HashMismatch
            | MissingAuthoredDir | StoreOrphan | NameCollision => Severity::Error,
            MissingClone | OrphanClone | DanglingLink | RemovedSkillLink | MissingConsumerDir
            | EmptyRegistration => Severity::Warning,
        }
    }

    /// A stable kebab-case label for display.
    pub fn as_str(self) -> &'static str {
        use FindingKind::*;
        match self {
            DeclaredButUnlocked => "declared-but-unlocked",
            StaleLock => "stale-lock",
            GlobDrift => "glob-drift",
            MissingClone => "missing-clone",
            CacheShaMismatch => "cache-sha-mismatch",
            HashMismatch => "hash-mismatch",
            OrphanClone => "orphan-clone",
            MissingAuthoredDir => "missing-authored-dir",
            StoreOrphan => "store-orphan",
            NameCollision => "name-collision",
            DanglingLink => "dangling-link",
            RemovedSkillLink => "removed-skill-link",
            MissingConsumerDir => "missing-consumer-dir",
            EmptyRegistration => "empty-registration",
        }
    }
}

/// One inconsistency: its kind, severity, subject, and a human detail. The
/// `severity` is serialized (not re-derived by adapters) so the CLI/GUI share
/// core's single source of truth for the error/warning split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    pub subject: String,
    pub detail: String,
}

impl Finding {
    fn new(kind: FindingKind, subject: impl Into<String>, detail: impl Into<String>) -> Self {
        Finding {
            kind,
            severity: kind.severity(),
            subject: subject.into(),
            detail: detail.into(),
        }
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }
}

/// The doctor report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Report {
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.findings
            .iter()
            .any(|f| f.severity() == Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn warning_count(&self) -> usize {
        self.count(Severity::Warning)
    }

    fn count(&self, sev: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity() == sev).count()
    }
}

/// How to run the check.
#[derive(Debug, Clone, Copy)]
pub struct DoctorOptions {
    /// Recompute per-skill content hashes (lock ↔ Cache integrity).
    pub verify: bool,
    /// Reconcile via sync/relink/prune before reporting the remainder.
    pub fix: bool,
    /// Include Consumer-link checks.
    pub consumers: bool,
}

impl Default for DoctorOptions {
    fn default() -> Self {
        DoctorOptions {
            verify: false,
            fix: false,
            consumers: true,
        }
    }
}

/// Run the integrity check, optionally reconciling first, and return findings.
pub fn doctor(sd: &Skilldock, opts: DoctorOptions) -> Result<Report> {
    if opts.fix {
        reconcile(sd)?;
    }

    let manifest = Manifest::read(&sd.manifest_path())?;
    let lock = Lock::read(&sd.lock_path())?;
    let mut findings = Vec::new();

    check_toml_lock(&manifest, &lock, &mut findings);
    check_lock_cache(sd, &manifest, &lock, opts.verify, &mut findings)?;
    check_store(sd, &manifest, &mut findings)?;
    check_collisions(&manifest, &lock, &mut findings);
    if opts.consumers {
        check_consumers(sd, &mut findings)?;
    }

    Ok(Report { findings })
}

/// `--fix`: materialize the Cache from the lock, then re-point and prune every
/// registered Consumer's links.
fn reconcile(sd: &Skilldock) -> Result<()> {
    ops::sync::sync(sd)?;
    for consumer in registry::read(sd)? {
        if consumer.is_dir() {
            let consumer = Consumer::Project(consumer);
            ops::relink::relink(sd, &consumer)?;
            ops::prune::prune(sd, &consumer)?;
        }
    }
    Ok(())
}

fn check_toml_lock(manifest: &Manifest, lock: &Lock, findings: &mut Vec<Finding>) {
    for v in &manifest.vendored {
        match lock.repos.iter().find(|r| r.repo == v.repo) {
            None => findings.push(Finding::new(
                FindingKind::DeclaredButUnlocked,
                &v.repo,
                "declared in toml but absent from the lock",
            )),
            Some(locked) => {
                for spec in &v.skills {
                    if let Some(name) = spec.link_name() {
                        if !locked.skills.iter().any(|s| s.name == name) {
                            findings.push(Finding::new(
                                FindingKind::DeclaredButUnlocked,
                                format!("{}#{name}", v.repo),
                                "explicit skill declared but absent from the lock",
                            ));
                        }
                    }
                }
            }
        }
    }
    for locked in &lock.repos {
        if manifest.vendored_repo(&locked.repo).is_none() {
            findings.push(Finding::new(
                FindingKind::StaleLock,
                &locked.repo,
                "in the lock but not declared in toml",
            ));
        }
    }
}

fn check_lock_cache(
    sd: &Skilldock,
    manifest: &Manifest,
    lock: &Lock,
    verify: bool,
    findings: &mut Vec<Finding>,
) -> Result<()> {
    for locked in &lock.repos {
        let clone = sd.cache_clone_dir(&locked.repo);
        if !clone.join(".git").is_dir() {
            findings.push(Finding::new(
                FindingKind::MissingClone,
                &locked.repo,
                "no Cache clone (run sync)",
            ));
            continue;
        }

        let head = git::rev_parse(&clone, "HEAD")?;
        if head != locked.resolved {
            findings.push(Finding::new(
                FindingKind::CacheShaMismatch,
                &locked.repo,
                format!("Cache at {head}, lock pins {}", locked.resolved),
            ));
            continue; // drift/hash unreliable against the wrong commit
        }

        // Glob drift: does the declaration still expand to the locked set?
        if let Some(v) = manifest.vendored_repo(&locked.repo) {
            match expand_skills(&clone, &v.skills) {
                Ok(current) => {
                    let now: BTreeSet<(&str, &str)> = current
                        .iter()
                        .map(|s| (s.name.as_str(), s.path.as_str()))
                        .collect();
                    let pinned: BTreeSet<(&str, &str)> = locked
                        .skills
                        .iter()
                        .map(|s| (s.name.as_str(), s.path.as_str()))
                        .collect();
                    if now != pinned {
                        findings.push(Finding::new(
                            FindingKind::GlobDrift,
                            &locked.repo,
                            "declaration re-expands to a different skill set than the lock",
                        ));
                    }
                }
                Err(_) => findings.push(Finding::new(
                    FindingKind::GlobDrift,
                    &locked.repo,
                    "declaration no longer resolves against the pinned commit",
                )),
            }
        }

        if verify {
            for skill in &locked.skills {
                let dir = clone.join(&skill.path);
                match hash_dir(&dir) {
                    Ok(h) if h == skill.hash => {}
                    _ => findings.push(Finding::new(
                        FindingKind::HashMismatch,
                        format!("{}#{}", locked.repo, skill.name),
                        "content hash differs from the lock",
                    )),
                }
            }
        }
    }

    for id in discover_clones(sd)? {
        if !lock.repos.iter().any(|r| r.repo == id) {
            findings.push(Finding::new(
                FindingKind::OrphanClone,
                id,
                "Cache clone not referenced by the lock",
            ));
        }
    }
    Ok(())
}

fn check_store(sd: &Skilldock, manifest: &Manifest, findings: &mut Vec<Finding>) -> Result<()> {
    for name in &manifest.authored {
        if !sd.authored_skill_dir(name).join("SKILL.md").is_file() {
            findings.push(Finding::new(
                FindingKind::MissingAuthoredDir,
                name,
                "authored skill missing (no SKILL.md) in the Store",
            ));
        }
    }

    let dir = sd.authored_skills_dir();
    for entry in read_dir_or_empty(&dir)? {
        let path = entry.path();
        if !path.join("SKILL.md").is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !manifest.authored.contains(&name) {
            findings.push(Finding::new(
                FindingKind::StoreOrphan,
                name,
                "skill in the Store not listed in `authored` (run author)",
            ));
        }
    }
    Ok(())
}

fn check_collisions(manifest: &Manifest, lock: &Lock, findings: &mut Vec<Finding>) {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for name in &manifest.authored {
        *counts.entry(name).or_default() += 1;
    }
    for repo in &lock.repos {
        for skill in &repo.skills {
            *counts.entry(&skill.name).or_default() += 1;
        }
    }
    for (name, n) in counts {
        if n > 1 {
            findings.push(Finding::new(
                FindingKind::NameCollision,
                name,
                format!("provided by {n} skills; an ambiguous link target"),
            ));
        }
    }
}

fn check_consumers(sd: &Skilldock, findings: &mut Vec<Finding>) -> Result<()> {
    let linkable: BTreeSet<String> = resolve::linkable(sd)?.into_iter().map(|l| l.name).collect();

    for consumer in registry::read(sd)? {
        if !consumer.is_dir() {
            findings.push(Finding::new(
                FindingKind::MissingConsumerDir,
                consumer.display().to_string(),
                "registered Consumer directory is gone",
            ));
            continue;
        }

        let skills_dir = consumer.join(".agents/skills");
        let mut linked = 0usize;
        for entry in read_dir_or_empty(&skills_dir)? {
            let path = entry.path();
            if !linkfs::is_symlink(&path) {
                continue;
            }
            linked += 1;
            let name = entry.file_name().to_string_lossy().into_owned();
            let subject = format!("{}: {name}", consumer.display());
            if linkfs::is_broken_symlink(&path) {
                findings.push(Finding::new(
                    FindingKind::DanglingLink,
                    subject,
                    "broken link",
                ));
            } else if !linkable.contains(&name) {
                findings.push(Finding::new(
                    FindingKind::RemovedSkillLink,
                    subject,
                    "links a skill that is no longer linkable",
                ));
            }
        }
        if linked == 0 {
            findings.push(Finding::new(
                FindingKind::EmptyRegistration,
                consumer.display().to_string(),
                "registered but holds no links",
            ));
        }
    }
    Ok(())
}

/// Identities of every Cache clone (a dir containing `.git`) under `cache/`.
fn discover_clones(sd: &Skilldock) -> Result<Vec<String>> {
    let cache = sd.cache();
    let mut out = Vec::new();
    walk_clones(&cache, &cache, &mut out)?;
    Ok(out)
}

fn walk_clones(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in read_dir_or_empty(dir)? {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(".git").is_dir() {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        } else {
            walk_clones(root, &path, out)?;
        }
    }
    Ok(())
}

/// Read a directory's entries, treating "not found" as empty. Any other read
/// error (permissions, etc.) propagates — never silently swallowed.
fn read_dir_or_empty(dir: &Path) -> Result<Vec<std::fs::DirEntry>> {
    match std::fs::read_dir(dir) {
        Ok(it) => it
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| Error::io(dir, e)),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::io(dir, e)),
    }
}
