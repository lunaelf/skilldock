//! Shared test harness for `skilldock-core` integration tests.
//!
//! Everything runs against a throwaway skilldock rooted at a temp directory and,
//! where git is involved, against local `git init` fixture Sources — no network,
//! fully parallel-safe (operations take an explicit `Skilldock`, so no env mutation).

#![allow(dead_code)] // helpers are shared across test binaries; not all use every one.

use std::path::{Path, PathBuf};
use std::process::Command;

use skilldock_core::Skilldock;
use tempfile::TempDir;

/// A throwaway skilldock: a temp root with the standard layout created.
/// Drops (and deletes) when it goes out of scope.
pub struct TempSkilldock {
    _tmp: TempDir,
    sd: Skilldock,
}

impl TempSkilldock {
    /// Create a fresh skilldock with `store/skills` and `cache/` in place.
    pub fn new() -> Self {
        let sd = Self::empty();
        sd.sd.ensure_layout().expect("ensure skilldock layout");
        sd
    }

    /// A skilldock whose root exists but is otherwise empty (no layout) — for
    /// `init`, which clones into an empty `store/`.
    pub fn empty() -> Self {
        let tmp = TempDir::new().expect("create tempdir");
        let sd = Skilldock::at(tmp.path());
        TempSkilldock { _tmp: tmp, sd }
    }

    pub fn sd(&self) -> &Skilldock {
        &self.sd
    }

    pub fn root(&self) -> &Path {
        self.sd.root()
    }
}

impl Default for TempSkilldock {
    fn default() -> Self {
        Self::new()
    }
}

/// A local git repo standing in for a vendored Source. Built with real `git`
/// so `add`/`sync` (later tickets) can clone it via `file://` with no network.
pub struct GitFixture {
    _tmp: TempDir,
    path: PathBuf,
}

impl GitFixture {
    /// Initialize an empty git repo in a fresh temp directory.
    pub fn init() -> Self {
        let tmp = TempDir::new().expect("create tempdir");
        let path = tmp.path().to_path_buf();
        git(&path, &["init", "-q"]);
        // Deterministic identity so commits succeed without global git config.
        git(&path, &["config", "user.email", "test@skilldock.local"]);
        git(&path, &["config", "user.name", "skilldock test"]);
        git(&path, &["config", "commit.gpgsign", "false"]);
        GitFixture { _tmp: tmp, path }
    }

    /// Write a skill with the given `SKILL.md` body at `subpath` (relative to
    /// the repo root). Parent directories are created.
    pub fn add_skill(&self, subpath: &str, skill_md: &str) -> &Self {
        self.write_file(&format!("{subpath}/SKILL.md"), skill_md)
    }

    /// Write an arbitrary file at `rel` (relative to the repo root), creating
    /// parent directories. For building non-skill fixtures (manifests, etc.).
    pub fn write_file(&self, rel: &str, content: &str) -> &Self {
        let path = self.path.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dir");
        }
        std::fs::write(path, content).expect("write file");
        self
    }

    /// Stage everything and commit; returns the resulting commit SHA.
    pub fn commit(&self, message: &str) -> String {
        git(&self.path, &["add", "-A"]);
        git(&self.path, &["commit", "-q", "-m", message]);
        self.head()
    }

    /// Tag the current `HEAD`.
    pub fn tag(&self, name: &str) -> &Self {
        git(&self.path, &["tag", name]);
        self
    }

    /// The current `HEAD` commit SHA.
    pub fn head(&self) -> String {
        git_head(&self.path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A `file://` URL usable as a clone remote.
    pub fn url(&self) -> String {
        format!("file://{}", self.path.display())
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("spawn git");
    assert!(
        status.success(),
        "git {:?} failed in {}",
        args,
        dir.display()
    );
}

/// The `HEAD` commit SHA of any git working tree (a clone or a fixture).
pub fn git_head(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("run git rev-parse");
    assert!(
        out.status.success(),
        "git rev-parse failed in {}",
        dir.display()
    );
    String::from_utf8(out.stdout)
        .expect("utf8 sha")
        .trim()
        .to_string()
}

/// The `git status --porcelain` output of a working tree (empty when clean).
pub fn git_status(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir)
        .output()
        .expect("run git status");
    assert!(
        out.status.success(),
        "git status failed in {}",
        dir.display()
    );
    String::from_utf8(out.stdout).expect("utf8 status")
}

/// A minimal valid `SKILL.md` body for fixtures.
pub fn skill_md(name: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: Fixture skill {name}.\n---\n\n# {name}\n\nFixture body.\n"
    )
}
