//! `migrate` — the one-shot conversion of the Bash-era three-manifest repo into
//! the new dock.
//!
//! It reads the old `skills-lock.json` + `authored.txt` + `external.json`,
//! synthesizes `skilldock.toml`, resolves every vendored Source at its current
//! upstream **HEAD** into `skilldock.lock` + the Cache, rebuilds the Store data
//! repo (authored originals + manifest + the pre-commit gate), backs up the old
//! manifests, and verifies the result with `doctor`. Re-pinning to HEAD may
//! adopt newer upstream skill versions, so a per-skill diff report surfaces
//! exactly which skills moved relative to the committed copies.
//!
//! CLI-only, like `init`. Without `--cleanup` the old repo is only ever read
//! (the manifest backup lives under the dock, not the old repo); the heavier
//! repo rename/data-repo split is the human cutover step. The one destructive
//! action — removing the superseded old manifests — is opt-in and runs only
//! after the built dock passes `doctor`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::expand::{basename, expand_skills};
use crate::hash::hash_dir;
use crate::lock::{Lock, LockRepo, LockSkill};
use crate::manifest::{Manifest, SkillSpec, VendoredRepo};
use crate::ops::doctor::{doctor, DoctorOptions, Report};
use crate::ops::init;
use crate::skilldock::Skilldock;
use crate::source::parse_source;
use crate::{cache, git};

/// The old manifest files, backed up before migration and removed by `--cleanup`.
const OLD_MANIFESTS: [&str; 3] = ["skills-lock.json", "authored.txt", "external.json"];

/// Subdirectory of the dock root the old manifests are backed up into (kept out
/// of the old repo so a migration without `--cleanup` never touches it).
const BACKUP_DIR: &str = "migrate-backup";

/// How a re-pinned vendored skill compares to the copy committed in the old repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillStatus {
    /// Content at HEAD matches the committed copy.
    Unchanged,
    /// Content at HEAD differs from the committed copy (re-pin adopted a change).
    Moved,
    /// No committed copy to compare against (new, or the copy was unreadable).
    Added,
    /// Declared in the old manifest but no longer present at upstream HEAD;
    /// dropped from the synthesized toml/lock.
    Dropped,
}

/// One entry in migrate's per-skill diff report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillReport {
    /// Source repo identity (`host/owner/repo`).
    pub repo: String,
    /// The skill's linked name.
    pub name: String,
    /// Subpath in the source repo (the declared subpath for a dropped skill).
    pub path: String,
    /// How the skill's HEAD content compares to the old committed copy.
    pub status: SkillStatus,
}

/// How to run the migration.
#[derive(Debug, Clone, Copy, Default)]
pub struct MigrateOptions {
    /// Compute and return the full plan without changing the dock: resolution
    /// clones into a throwaway cache (discarded before returning), so neither
    /// the Store, the Cache, nor the old repo is touched.
    pub dry_run: bool,
    /// After the built dock passes `doctor`, remove the superseded old manifest
    /// files from the old repo. Off by default — the repo rename/split is the
    /// human cutover step.
    pub cleanup: bool,
}

/// What `migrate` produced (or, in `--dry-run`, would produce).
#[derive(Debug, Clone, Serialize)]
pub struct MigrateOutcome {
    /// The synthesized declared manifest (`skilldock.toml`).
    pub manifest: Manifest,
    /// The resolved lock (`skilldock.lock`), pinned to upstream HEAD.
    pub lock: Lock,
    /// Per-skill diff report, sorted by (repo, name).
    pub skills: Vec<SkillReport>,
    /// Authored skill names copied into the Store.
    pub authored: Vec<String>,
    /// True when this was a dry run (nothing was written).
    pub dry_run: bool,
    /// The `doctor` report over the freshly built dock (`None` in `--dry-run`).
    pub doctor: Option<Report>,
    /// Where the old manifests were backed up (`None` in `--dry-run`).
    pub backup: Option<PathBuf>,
    /// Whether the old manifest files were removed from the old repo.
    pub cleaned: bool,
}

/// Convert the old three-manifest repo at `old_repo` into a fresh dock at `sd`.
pub fn migrate(sd: &Skilldock, old_repo: &Path, opts: MigrateOptions) -> Result<MigrateOutcome> {
    // Refuse to clobber an already-populated dock.
    if sd.manifest_path().exists() || sd.lock_path().exists() {
        return Err(Error::Invalid(format!(
            "{} already holds a skilldock.toml/lock; migrate builds a fresh Store",
            sd.store().display()
        )));
    }

    // Safety rail: a real migration requires a clean git tree in the old repo.
    // A dry run is a read-only preview, so it tolerates a dirty tree.
    if !opts.dry_run {
        require_clean_tree(old_repo)?;
    }

    let requested = read_old_manifests(old_repo)?;
    // A dry run resolves against a throwaway cache so the real dock is untouched;
    // a real run resolves into (and keeps) the dock's Cache.
    let scratch = if opts.dry_run {
        Some(ScratchDock::new()?)
    } else {
        None
    };
    let resolve_sd = scratch.as_ref().map(|s| &s.sd).unwrap_or(sd);
    let plan = resolve_sources(resolve_sd, old_repo, &requested)?;

    let outcome = MigrateOutcome {
        manifest: plan.manifest.clone(),
        lock: plan.lock.clone(),
        skills: plan.skills.clone(),
        authored: requested.authored.clone(),
        dry_run: opts.dry_run,
        doctor: None,
        backup: None,
        cleaned: false,
    };

    if opts.dry_run {
        return Ok(outcome);
    }

    // Back up the old manifests (a copy under the dock — the old repo is untouched).
    let backup = backup_old_manifests(sd, old_repo)?;

    // Build the Store, then verify before any destructive step.
    build_store(sd, old_repo, &plan, &requested.authored)?;
    let report = doctor(sd, DoctorOptions::default())?;

    // Destructive cleanup runs only after the new dock passes doctor (opt-in).
    let cleaned = if opts.cleanup && !report.has_errors() {
        remove_old_manifests(old_repo)?;
        true
    } else {
        false
    };

    Ok(MigrateOutcome {
        doctor: Some(report),
        backup: Some(backup),
        cleaned,
        ..outcome
    })
}

// --- old-manifest reading -------------------------------------------------

/// The old `skills-lock.json` (npx-installed vendored skills).
#[derive(Debug, Deserialize)]
struct OldLock {
    #[serde(default)]
    skills: BTreeMap<String, OldLockEntry>,
}

#[derive(Debug, Deserialize)]
struct OldLockEntry {
    source: String,
    #[serde(default, rename = "skillPath")]
    skill_path: String,
}

/// The old `external.json` (GitHub-repo symlinked externals).
#[derive(Debug, Deserialize)]
struct OldExternal {
    #[serde(default)]
    skills: BTreeMap<String, OldExternalEntry>,
}

#[derive(Debug, Deserialize)]
struct OldExternalEntry {
    repo: String,
    #[serde(default, rename = "ref")]
    git_ref: String,
    #[serde(default, rename = "skillPath")]
    skill_path: String,
}

/// One vendored source repo requested by the old manifests, keyed by identity.
struct RequestedRepo {
    identity: String,
    url: String,
    git_ref: Option<String>,
    skills: Vec<RequestedSkill>,
}

/// One skill requested from a source repo: its name and directory subpath.
struct RequestedSkill {
    name: String,
    path: String,
}

/// Everything the old manifests declared: vendored source repos + authored names.
struct Requested {
    repos: Vec<RequestedRepo>,
    authored: Vec<String>,
}

/// Read and merge the three old manifests into a canonical requested set.
fn read_old_manifests(old_repo: &Path) -> Result<Requested> {
    let mut by_id: BTreeMap<String, RequestedRepo> = BTreeMap::new();

    if let Some(text) = read_optional(&old_repo.join("skills-lock.json"))? {
        let parsed: OldLock = parse_json(&text, "skills-lock.json")?;
        for (name, entry) in parsed.skills {
            insert_requested(&mut by_id, &name, &entry.source, None, &entry.skill_path)?;
        }
    }

    if let Some(text) = read_optional(&old_repo.join("external.json"))? {
        let parsed: OldExternal = parse_json(&text, "external.json")?;
        for (name, entry) in parsed.skills {
            let git_ref = non_empty(&entry.git_ref);
            insert_requested(&mut by_id, &name, &entry.repo, git_ref, &entry.skill_path)?;
        }
    }

    let mut repos: Vec<RequestedRepo> = by_id.into_values().collect();
    for repo in &mut repos {
        repo.skills.sort_by(|a, b| a.name.cmp(&b.name));
        repo.skills
            .dedup_by(|a, b| a.name == b.name && a.path == b.path);
    }

    let authored = read_authored(&old_repo.join("authored.txt"))?;
    Ok(Requested { repos, authored })
}

/// Merge one requested skill into the by-identity map, parsing its raw source
/// (an `owner/repo` shorthand or a full URL) into a canonical identity + URL.
fn insert_requested(
    by_id: &mut BTreeMap<String, RequestedRepo>,
    name: &str,
    raw_source: &str,
    git_ref: Option<String>,
    skill_path: &str,
) -> Result<()> {
    let source = parse_source(raw_source)?;
    let repo = by_id
        .entry(source.repo.clone())
        .or_insert_with(|| RequestedRepo {
            identity: source.repo.clone(),
            url: source.url.clone(),
            git_ref: None,
            skills: Vec::new(),
        });
    // First explicit ref wins (skills-lock carries none; external may).
    if repo.git_ref.is_none() {
        repo.git_ref = git_ref;
    }
    repo.skills.push(RequestedSkill {
        name: name.to_string(),
        path: skill_dir_from_path(skill_path),
    });
    Ok(())
}

/// Turn an old `skillPath` into the skill's directory subpath. Old entries point
/// either at the skill's `SKILL.md` (npx lock) or directly at its directory
/// (external); a path resolving to the repo root becomes ".".
fn skill_dir_from_path(skill_path: &str) -> String {
    let p = skill_path.trim();
    let p = p
        .strip_suffix("/SKILL.md")
        .or_else(|| (p == "SKILL.md").then_some(""))
        .unwrap_or(p);
    let p = p.trim_matches('/');
    if p.is_empty() {
        ".".to_string()
    } else {
        p.to_string()
    }
}

/// Parse `authored.txt`: one name per line, ignoring blanks and `#` comments.
fn read_authored(path: &Path) -> Result<Vec<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let mut names: Vec<String> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_string)
                .collect();
            names.sort();
            names.dedup();
            Ok(names)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(Error::io(path, e)),
    }
}

// --- source resolution ----------------------------------------------------

/// The synthesized manifest, resolved lock, and per-skill diff report.
struct Plan {
    manifest: Manifest,
    lock: Lock,
    skills: Vec<SkillReport>,
}

/// Clone every requested Source, pin its upstream HEAD, expand its skills into
/// hashed lock entries, and classify each against the old committed copy.
fn resolve_sources(sd: &Skilldock, old_repo: &Path, requested: &Requested) -> Result<Plan> {
    let mut manifest = Manifest {
        authored: requested.authored.clone(),
        vendored: Vec::new(),
    };
    let mut lock = Lock::default();
    let mut reports: Vec<SkillReport> = Vec::new();

    for req in &requested.repos {
        // A one-shot migration re-pins to current upstream HEAD, so always start
        // from a pristine clone — a `fetch` advances only remote-tracking refs,
        // not a reused clone's HEAD, so drop any leftover and clone fresh.
        cache::remove_clone(sd, &req.identity)?;
        let (clone_dir, _) = cache::ensure_clone(sd, &req.identity, &req.url)?;
        if let Some(git_ref) = &req.git_ref {
            git::checkout(&clone_dir, git_ref)?;
        }
        let resolved = git::rev_parse(&clone_dir, "HEAD")?;

        let mut specs: Vec<SkillSpec> = Vec::new();
        let mut skills: BTreeMap<String, LockSkill> = BTreeMap::new();

        for rs in &req.skills {
            let spec = spec_for(&rs.name, &rs.path);
            match expand_skills(&clone_dir, std::slice::from_ref(&spec)) {
                Ok(resolved_skills) => {
                    specs.push(spec);
                    for ls in resolved_skills {
                        reports.push(SkillReport {
                            repo: req.identity.clone(),
                            name: ls.name.clone(),
                            path: ls.path.clone(),
                            status: diff_status(old_repo, &ls),
                        });
                        skills.insert(ls.path.clone(), ls);
                    }
                }
                // The declared skill no longer exists at HEAD — drop it, but
                // record it so the report shows what was lost.
                Err(_) => reports.push(SkillReport {
                    repo: req.identity.clone(),
                    name: rs.name.clone(),
                    path: rs.path.clone(),
                    status: SkillStatus::Dropped,
                }),
            }
        }

        if skills.is_empty() {
            // Every declared skill vanished — remove the clone so it isn't an orphan.
            cache::remove_clone(sd, &req.identity)?;
            continue;
        }

        let mut lock_skills: Vec<LockSkill> = skills.into_values().collect();
        lock_skills.sort_by(|a, b| a.name.cmp(&b.name));

        manifest.vendored.push(VendoredRepo {
            repo: req.identity.clone(),
            git_ref: req.git_ref.clone(),
            skills: specs,
        });
        lock.upsert_repo(LockRepo {
            repo: req.identity.clone(),
            url: req.url.clone(),
            resolved,
            skills: lock_skills,
        });
    }

    manifest.vendored.sort_by(|a, b| a.repo.cmp(&b.repo));
    reports.sort_by(|a, b| (&a.repo, &a.name).cmp(&(&b.repo, &b.name)));

    Ok(Plan {
        manifest,
        lock,
        skills: reports,
    })
}

/// Build the toml spec for a skill: a bare path when its basename already yields
/// the name, else a `{name, path}` rename (repo-root skills always rename,
/// since their basename is ".").
fn spec_for(name: &str, path: &str) -> SkillSpec {
    if basename(path) == name {
        SkillSpec::Path(path.to_string())
    } else {
        SkillSpec::Named {
            name: name.to_string(),
            path: path.to_string(),
        }
    }
}

/// Compare a re-pinned skill's HEAD content hash to the copy committed under the
/// old repo's `.agents/skills/<name>`.
fn diff_status(old_repo: &Path, skill: &LockSkill) -> SkillStatus {
    let committed = old_repo.join(".agents/skills").join(&skill.name);
    if !committed.is_dir() {
        return SkillStatus::Added;
    }
    match hash_dir(&committed) {
        Ok(old_hash) if old_hash == skill.hash => SkillStatus::Unchanged,
        Ok(_) => SkillStatus::Moved,
        // Committed copy unreadable (e.g. a dangling external symlink).
        Err(_) => SkillStatus::Added,
    }
}

// --- store construction + safety rails ------------------------------------

/// Write the manifest + lock into the Store, copy authored originals from the
/// old repo, `git init` the Store, and install the pre-commit gate.
fn build_store(sd: &Skilldock, old_repo: &Path, plan: &Plan, authored: &[String]) -> Result<()> {
    let skills_dir = sd.authored_skills_dir();
    std::fs::create_dir_all(&skills_dir).map_err(|e| Error::io(&skills_dir, e))?;

    plan.manifest.write(&sd.manifest_path())?;
    plan.lock.write(&sd.lock_path())?;

    for name in authored {
        let src = old_repo.join(".agents/skills").join(name);
        if !src.join("SKILL.md").is_file() {
            return Err(Error::Invalid(format!(
                "authored skill '{name}' has no SKILL.md at {}",
                src.display()
            )));
        }
        copy_dir_all(&src, &sd.authored_skill_dir(name))?;
    }

    git::init(&sd.store())?;
    init::install_pre_commit_hook(sd)?;
    Ok(())
}

/// Copy the old manifests into a backup directory under the dock root.
fn backup_old_manifests(sd: &Skilldock, old_repo: &Path) -> Result<PathBuf> {
    let dir = sd.root().join(BACKUP_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    for name in OLD_MANIFESTS {
        let src = old_repo.join(name);
        if src.is_file() {
            let dst = dir.join(name);
            std::fs::copy(&src, &dst).map_err(|e| Error::io(&dst, e))?;
        }
    }
    Ok(dir)
}

/// Remove the superseded old manifest files from the old repo (post-verify).
fn remove_old_manifests(old_repo: &Path) -> Result<()> {
    for name in OLD_MANIFESTS {
        let path = old_repo.join(name);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io(&path, e)),
        }
    }
    Ok(())
}

/// Refuse to migrate a dirty old repo — the one-shot conversion wants a known
/// git state to fall back to.
fn require_clean_tree(old_repo: &Path) -> Result<()> {
    let status = git::status_porcelain(old_repo)?;
    if !status.trim().is_empty() {
        return Err(Error::Invalid(format!(
            "old repo {} has uncommitted changes; commit or stash them before migrating",
            old_repo.display()
        )));
    }
    Ok(())
}

/// Recursively copy `src` into `dst`, skipping any nested `.git`.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| Error::io(dst, e))?;
    for entry in std::fs::read_dir(src).map_err(|e| Error::io(src, e))? {
        let entry = entry.map_err(|e| Error::io(src, e))?;
        if entry.file_name() == ".git" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry.file_type().map_err(|e| Error::io(&from, e))?;
        if file_type.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| Error::io(&to, e))?;
        }
    }
    Ok(())
}

/// A throwaway dock rooted in a unique temp directory, used to resolve a
/// `--dry-run` plan without cloning into (or otherwise touching) the real dock.
/// Its directory is removed when the guard drops.
struct ScratchDock {
    sd: Skilldock,
    root: PathBuf,
}

impl ScratchDock {
    fn new() -> Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        // Unique across threads (counter) and processes (pid + nanos) so parallel
        // dry runs never share a scratch cache.
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "skilldock-migrate-dryrun-{}-{nanos}-{n}",
            std::process::id()
        ));

        let sd = Skilldock::at(&root);
        std::fs::create_dir_all(sd.cache()).map_err(|e| Error::io(sd.cache(), e))?;
        Ok(ScratchDock { sd, root })
    }
}

impl Drop for ScratchDock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

// --- small file helpers ---------------------------------------------------

fn read_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

fn parse_json<T: for<'de> Deserialize<'de>>(text: &str, name: &str) -> Result<T> {
    serde_json::from_str(text).map_err(|e| Error::Invalid(format!("failed to parse {name}: {e}")))
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}
