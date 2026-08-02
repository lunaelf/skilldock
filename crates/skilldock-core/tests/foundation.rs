//! Foundational behavior: dock layout, manifest/lock round-trips, the
//! globs-only-in-toml rule, and that the git fixture harness works.

mod common;

use common::{skill_md, GitFixture, TempSkilldock};
use skilldock_core::{Lock, LockRepo, LockSkill, Manifest, SkillSpec, Skilldock, VendoredRepo};

#[test]
fn paths_resolve_under_root() {
    let sd = Skilldock::at("/tmp/example-dock");
    assert_eq!(sd.store(), std::path::Path::new("/tmp/example-dock/store"));
    assert_eq!(sd.cache(), std::path::Path::new("/tmp/example-dock/cache"));
    assert_eq!(
        sd.config_path(),
        std::path::Path::new("/tmp/example-dock/config.toml")
    );
    assert_eq!(
        sd.manifest_path(),
        std::path::Path::new("/tmp/example-dock/store/skilldock.toml")
    );
    assert_eq!(
        sd.lock_path(),
        std::path::Path::new("/tmp/example-dock/store/skilldock.lock")
    );
    assert_eq!(
        sd.cache_clone_dir("github.com/mattpocock/skills"),
        std::path::Path::new("/tmp/example-dock/cache/github.com/mattpocock/skills")
    );
}

#[test]
fn ensure_layout_creates_store_and_cache() {
    let d = TempSkilldock::new();
    assert!(d.sd().authored_skills_dir().is_dir());
    assert!(d.sd().cache().is_dir());
}

#[test]
fn manifest_round_trips_all_declared_forms() {
    // A manifest that exercises every declared skill form: bare path, glob,
    // and a {name, path} rename — plus an authored entry and a bare ref.
    let manifest = Manifest {
        authored: vec!["git-commit".into()],
        vendored: vec![VendoredRepo {
            repo: "github.com/mattpocock/skills".into(),
            git_ref: Some("main".into()),
            skills: vec![
                SkillSpec::Path("skills/engineering/grilling".into()),
                SkillSpec::Path("skills/engineering/*".into()),
                SkillSpec::Named {
                    name: "dm".into(),
                    path: "skills/engineering/domain-modeling".into(),
                },
            ],
        }],
    };

    let text = manifest.to_toml().expect("serialize manifest");
    let back = Manifest::parse(&text, std::path::Path::new("skilldock.toml")).expect("parse");
    assert_eq!(manifest, back, "manifest did not round-trip:\n{text}");

    // Spot-check the glob classification survived.
    let globs: Vec<_> = back.vendored[0]
        .skills
        .iter()
        .filter(|s| s.is_glob())
        .map(|s| s.path().to_string())
        .collect();
    assert_eq!(globs, vec!["skills/engineering/*".to_string()]);
}

#[test]
fn manifest_omits_ref_when_absent() {
    let manifest = Manifest {
        authored: vec![],
        vendored: vec![VendoredRepo {
            repo: "github.com/a/b".into(),
            git_ref: None,
            skills: vec![SkillSpec::Path("s".into())],
        }],
    };
    let text = manifest.to_toml().unwrap();
    assert!(!text.contains("ref"), "ref should be omitted:\n{text}");
    let back = Manifest::parse(&text, std::path::Path::new("m.toml")).unwrap();
    assert_eq!(manifest, back);
}

#[test]
fn lock_round_trips_exact_entries() {
    let lock = Lock {
        repos: vec![LockRepo {
            repo: "github.com/mattpocock/skills".into(),
            url: "https://github.com/mattpocock/skills.git".into(),
            resolved: "a1b2c3d4e5f6".into(),
            skills: vec![LockSkill {
                name: "grilling".into(),
                path: "skills/engineering/grilling".into(),
                hash: "sha256:deadbeef".into(),
            }],
        }],
    };
    let text = lock.to_toml().expect("serialize lock");
    let back = Lock::parse(&text, std::path::Path::new("skilldock.lock")).expect("parse");
    assert_eq!(lock, back, "lock did not round-trip:\n{text}");
}

#[test]
fn lock_rejects_a_glob_path() {
    let lock = Lock {
        repos: vec![LockRepo {
            repo: "github.com/a/b".into(),
            url: String::new(),
            resolved: "abc".into(),
            skills: vec![LockSkill {
                name: "x".into(),
                path: "skills/engineering/*".into(),
                hash: "sha256:0".into(),
            }],
        }],
    };
    let err = lock
        .to_toml()
        .expect_err("glob path must be rejected in the lock");
    assert!(
        matches!(err, skilldock_core::Error::GlobInLock(ref p) if p == "skills/engineering/*"),
        "unexpected error: {err}"
    );
}

#[test]
fn manifest_read_missing_is_empty() {
    let d = TempSkilldock::new();
    let m = Manifest::read(&d.sd().manifest_path()).unwrap();
    assert_eq!(m, Manifest::default());
}

#[test]
fn git_fixture_builds_a_committed_repo() {
    let repo = GitFixture::init();
    repo.add_skill("skills/grilling", &skill_md("grilling"));
    let sha = repo.commit("add grilling");
    assert_eq!(sha.len(), 40, "expected a full commit SHA, got {sha:?}");
    assert!(repo.path().join("skills/grilling/SKILL.md").is_file());
    assert!(repo.url().starts_with("file://"));
}
