//! `list` groups skills by provenance: authored from the manifest (with a
//! presence flag from the Store), vendored from the resolved lock.

mod common;

use common::TempSkilldock;
use skilldock_core::{list, Lock, LockRepo, LockSkill, Manifest};

#[test]
fn empty_dock_lists_nothing() {
    let d = TempSkilldock::new();
    let listing = list(d.sd()).unwrap();
    assert!(listing.authored.is_empty());
    assert!(listing.vendored.is_empty());
}

#[test]
fn authored_come_from_manifest_with_presence() {
    let d = TempSkilldock::new();
    // Two authored names; only one has a directory in the Store.
    let manifest = Manifest {
        authored: vec!["git-commit".into(), "ghost".into()],
        vendored: vec![],
    };
    manifest.write(&d.sd().manifest_path()).unwrap();
    std::fs::create_dir_all(d.sd().authored_skill_dir("git-commit")).unwrap();

    let listing = list(d.sd()).unwrap();
    let names: Vec<_> = listing.authored.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["ghost", "git-commit"]); // sorted

    let present: std::collections::BTreeMap<_, _> = listing
        .authored
        .iter()
        .map(|s| (s.name.as_str(), s.present))
        .collect();
    assert!(present["git-commit"]);
    assert!(!present["ghost"]);
}

#[test]
fn vendored_come_from_the_lock() {
    let d = TempSkilldock::new();
    let lock = Lock {
        repos: vec![LockRepo {
            repo: "github.com/mattpocock/skills".into(),
            resolved: "a1b2c3d4".into(),
            skills: vec![
                LockSkill {
                    name: "grilling".into(),
                    path: "skills/engineering/grilling".into(),
                    hash: "sha256:aa".into(),
                },
                LockSkill {
                    name: "dm".into(),
                    path: "skills/engineering/domain-modeling".into(),
                    hash: "sha256:bb".into(),
                },
            ],
        }],
    };
    lock.write(&d.sd().lock_path()).unwrap();

    let listing = list(d.sd()).unwrap();
    let names: Vec<_> = listing.vendored.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["dm", "grilling"]); // sorted by name
    let g = listing
        .vendored
        .iter()
        .find(|s| s.name == "grilling")
        .unwrap();
    assert_eq!(g.repo, "github.com/mattpocock/skills");
    assert_eq!(g.path, "skills/engineering/grilling");
    assert_eq!(g.resolved, "a1b2c3d4");
}
