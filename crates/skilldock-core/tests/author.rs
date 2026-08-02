//! `author` scaffolds or marks an authored skill and records it in the manifest.

mod common;

use common::TempSkilldock;
use skilldock_core::{author, Manifest};

#[test]
fn scaffolds_a_new_skill_and_records_it() {
    let d = TempSkilldock::new();
    let outcome = author(d.sd(), "my-skill").unwrap();

    assert_eq!(outcome.name, "my-skill");
    assert!(outcome.scaffolded, "a new skill should be scaffolded");
    assert!(!outcome.already_listed);

    // SKILL.md exists and is a minimally valid skill (has a name front-matter).
    let skill_md = d.sd().authored_skill_dir("my-skill").join("SKILL.md");
    let body = std::fs::read_to_string(&skill_md).unwrap();
    assert!(body.contains("name: my-skill"), "scaffold body:\n{body}");

    // Recorded in the manifest's authored list.
    let manifest = Manifest::read(&d.sd().manifest_path()).unwrap();
    assert_eq!(manifest.authored, vec!["my-skill".to_string()]);
}

#[test]
fn marks_an_existing_skill_without_overwriting() {
    let d = TempSkilldock::new();
    let dir = d.sd().authored_skill_dir("existing");
    std::fs::create_dir_all(&dir).unwrap();
    let original = "---\nname: existing\ndescription: Hand-written.\n---\n\nreal content\n";
    std::fs::write(dir.join("SKILL.md"), original).unwrap();

    let outcome = author(d.sd(), "existing").unwrap();
    assert!(!outcome.scaffolded, "existing skill must not be scaffolded");
    assert!(!outcome.already_listed);

    // Content is untouched.
    let body = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
    assert_eq!(body, original);

    let manifest = Manifest::read(&d.sd().manifest_path()).unwrap();
    assert_eq!(manifest.authored, vec!["existing".to_string()]);
}

#[test]
fn is_idempotent() {
    let d = TempSkilldock::new();
    author(d.sd(), "dup").unwrap();
    let second = author(d.sd(), "dup").unwrap();

    assert!(second.already_listed, "second call should see it listed");
    assert!(
        !second.scaffolded,
        "already-scaffolded skill isn't re-scaffolded"
    );

    let manifest = Manifest::read(&d.sd().manifest_path()).unwrap();
    assert_eq!(
        manifest.authored,
        vec!["dup".to_string()],
        "no duplicate entry"
    );
}

#[test]
fn preserves_other_manifest_entries() {
    let d = TempSkilldock::new();
    // Pre-existing manifest with vendored + one authored entry.
    let text = r#"authored = ["git-commit"]

[[vendored]]
repo = "github.com/a/b"
skills = ["skills/x"]
"#;
    std::fs::write(d.sd().manifest_path(), text).unwrap();

    author(d.sd(), "new-one").unwrap();

    let manifest = Manifest::read(&d.sd().manifest_path()).unwrap();
    assert_eq!(
        manifest.authored,
        vec!["git-commit".to_string(), "new-one".to_string()]
    );
    assert_eq!(manifest.vendored.len(), 1, "vendored section preserved");
    assert_eq!(manifest.vendored[0].repo, "github.com/a/b");
}

#[test]
fn rejects_invalid_names() {
    let d = TempSkilldock::new();
    for bad in ["", "a/b", "..", ".", "with space", "../escape"] {
        assert!(
            author(d.sd(), bad).is_err(),
            "name {bad:?} should be rejected"
        );
    }
    // Nothing leaked into the manifest.
    let manifest = Manifest::read(&d.sd().manifest_path()).unwrap();
    assert!(manifest.authored.is_empty());
}
