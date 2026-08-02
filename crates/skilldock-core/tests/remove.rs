//! `remove` drops a vendored skill/repo from the manifest + lock and prunes the
//! Cache clone once nothing references it.

mod common;

use common::{skill_md, GitFixture, TempSkilldock};
use skilldock_core::{add, remove, AddRequest, Lock, Manifest, SkillSpec, Skilldock, Source};

/// Add a two-skill repo declared with explicit paths.
fn setup_two_skill_repo(sd: &Skilldock) -> GitFixture {
    let repo = GitFixture::init();
    repo.add_skill("a/one", &skill_md("one"));
    repo.add_skill("a/two", &skill_md("two"));
    repo.commit("seed");
    add(
        sd,
        AddRequest {
            source: Source {
                repo: "local/test/proj".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![
                SkillSpec::Path("a/one".into()),
                SkillSpec::Path("a/two".into()),
            ],
        },
    )
    .unwrap();
    repo
}

#[test]
fn remove_one_skill_leaves_the_others_and_keeps_the_clone() {
    let sd = TempSkilldock::new();
    let _repo = setup_two_skill_repo(sd.sd());

    let out = remove(sd.sd(), "one").unwrap();
    assert_eq!(out.removed, vec!["one"]);
    assert!(out.pruned_clones.is_empty(), "repo still has a skill");

    let manifest = Manifest::read(&sd.sd().manifest_path()).unwrap();
    let specs: Vec<_> = manifest.vendored[0]
        .skills
        .iter()
        .map(|s| s.path().to_string())
        .collect();
    assert_eq!(specs, vec!["a/two"], "only the removed spec is dropped");

    let lock = Lock::read(&sd.sd().lock_path()).unwrap();
    let names: Vec<_> = lock.repos[0]
        .skills
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(names, vec!["two"]);

    assert!(
        sd.sd()
            .cache_clone_dir("local/test/proj")
            .join(".git")
            .is_dir(),
        "clone kept while still referenced"
    );
}

#[test]
fn removing_the_last_skill_drops_the_repo_and_prunes_the_clone() {
    let sd = TempSkilldock::new();
    let _repo = setup_two_skill_repo(sd.sd());

    remove(sd.sd(), "one").unwrap();
    let out = remove(sd.sd(), "two").unwrap();
    assert_eq!(out.pruned_clones, vec!["local/test/proj"]);

    let manifest = Manifest::read(&sd.sd().manifest_path()).unwrap();
    assert!(manifest.vendored.is_empty(), "repo dropped from toml");
    let lock = Lock::read(&sd.sd().lock_path()).unwrap();
    assert!(lock.repos.is_empty(), "repo dropped from lock");
    assert!(
        !sd.sd().cache_clone_dir("local/test/proj").exists(),
        "clone pruned"
    );
}

#[test]
fn remove_by_repo_identity_drops_everything_and_prunes() {
    let sd = TempSkilldock::new();
    let _repo = setup_two_skill_repo(sd.sd());

    let out = remove(sd.sd(), "local/test/proj").unwrap();
    assert_eq!(out.removed, vec!["local/test/proj"]);
    assert_eq!(out.pruned_clones, vec!["local/test/proj"]);

    assert!(Manifest::read(&sd.sd().manifest_path())
        .unwrap()
        .vendored
        .is_empty());
    assert!(Lock::read(&sd.sd().lock_path()).unwrap().repos.is_empty());
    assert!(!sd.sd().cache_clone_dir("local/test/proj").exists());
}

#[test]
fn removing_a_glob_provided_skill_is_refused() {
    let sd = TempSkilldock::new();
    let repo = GitFixture::init();
    repo.add_skill("a/one", &skill_md("one"));
    repo.add_skill("a/two", &skill_md("two"));
    repo.commit("seed");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/glob".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("a/*".into())],
        },
    )
    .unwrap();

    // Can't cleanly drop one member of a glob — the user is directed at the repo.
    let err = remove(sd.sd(), "one");
    assert!(err.is_err(), "glob-provided skill can't be removed by name");
    // Nothing was mutated.
    assert_eq!(
        Lock::read(&sd.sd().lock_path()).unwrap().repos[0]
            .skills
            .len(),
        2
    );
}

#[test]
fn removing_an_unknown_name_errors() {
    let sd = TempSkilldock::new();
    assert!(remove(sd.sd(), "nope").is_err());
}
