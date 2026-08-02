//! `add` declares a vendored source, clones it into the Cache pinned to one
//! commit, expands globs, and records the resolved SHA + per-skill hashes.

mod common;

use common::{skill_md, GitFixture, TempSkilldock};
use skilldock_core::{add, AddRequest, Lock, Manifest, SkillSpec, Source};

/// A fixture Source with three skills: one standalone and two under a subtree.
fn fixture() -> (GitFixture, String) {
    let repo = GitFixture::init();
    repo.add_skill("skills/grilling", &skill_md("grilling"));
    repo.add_skill(
        "skills/engineering/domain-modeling",
        &skill_md("domain-modeling"),
    );
    repo.add_skill("skills/engineering/prototype", &skill_md("prototype"));
    // A non-skill dir under the glob must be ignored.
    std::fs::create_dir_all(repo.path().join("skills/engineering/notes")).unwrap();
    let sha = repo.commit("seed skills");
    (repo, sha)
}

#[test]
fn add_declares_clones_pins_and_hashes() {
    let sd = TempSkilldock::new();
    let (repo, sha) = fixture();

    let outcome = add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/skills".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![
                SkillSpec::Path("skills/grilling".into()),
                SkillSpec::Path("skills/engineering/*".into()),
            ],
        },
    )
    .unwrap();

    // Pinned to the fixture's HEAD.
    assert_eq!(outcome.resolved, sha);

    // Glob expanded to the two engineering skills + the standalone one.
    let names: Vec<_> = outcome.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["domain-modeling", "grilling", "prototype"]);
    assert!(outcome.skills.iter().all(|s| s.hash.starts_with("sha256:")));

    // Manifest: one [[vendored]] entry with the declared specs (globs preserved).
    let manifest = Manifest::read(&sd.sd().manifest_path()).unwrap();
    assert_eq!(manifest.vendored.len(), 1);
    assert_eq!(manifest.vendored[0].repo, "local/test/skills");
    assert!(manifest.vendored[0]
        .skills
        .contains(&SkillSpec::Path("skills/engineering/*".into())));

    // Lock: one repo, single SHA for all skills, url recorded, exact paths only.
    let lock = Lock::read(&sd.sd().lock_path()).unwrap();
    assert_eq!(lock.repos.len(), 1, "all skills pin to one repo entry");
    let locked = &lock.repos[0];
    assert_eq!(locked.repo, "local/test/skills");
    assert_eq!(locked.url, repo.url());
    assert_eq!(locked.resolved, sha);
    assert_eq!(locked.skills.len(), 3);
    assert!(locked.skills.iter().all(|s| !s.path.contains('*')));

    // Cache: a real clone with the skill files present.
    let clone = sd.sd().cache_clone_dir("local/test/skills");
    assert!(clone.join(".git").is_dir());
    assert!(clone.join("skills/grilling/SKILL.md").is_file());
    assert!(clone
        .join("skills/engineering/domain-modeling/SKILL.md")
        .is_file());
}

#[test]
fn add_pins_to_a_tag_ref() {
    let sd = TempSkilldock::new();
    let repo = GitFixture::init();
    repo.add_skill("s/one", &skill_md("one"));
    let tagged = repo.commit("first");
    repo.tag("v1");
    // A later commit that the tag must NOT resolve to.
    repo.add_skill("s/two", &skill_md("two"));
    repo.commit("second");

    let outcome = add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/tagged".into(),
                url: repo.url(),
            },
            git_ref: Some("v1".into()),
            skills: vec![SkillSpec::Path("s/one".into())],
        },
    )
    .unwrap();

    assert_eq!(outcome.resolved, tagged, "ref pins to the tagged commit");
}
