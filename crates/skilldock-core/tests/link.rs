//! Consumer linking: link/unlink/prune/relink/register against a temp Consumer,
//! asserting on the resulting symlinks and `links.txt`.

mod common;

use std::path::Path;

use common::{skill_md, GitFixture, TempSkilldock};
use skilldock_core::{
    add, author, deregister, link, prune, register, relink, unlink, AddRequest, Consumer,
    SkillSpec, Source,
};

/// A skilldock with one authored skill (`git-commit`) and one vendored skill
/// (`grilling` from `local/test/skills`).
fn setup() -> TempSkilldock {
    let sd = TempSkilldock::new();
    author(sd.sd(), "git-commit").unwrap();

    let repo = GitFixture::init();
    repo.add_skill("skills/grilling", &skill_md("grilling"));
    repo.commit("seed");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/skills".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("skills/grilling".into())],
        },
    )
    .unwrap();
    sd
}

fn registry_lines(sd: &skilldock_core::Skilldock) -> Vec<String> {
    match std::fs::read_to_string(sd.links_path()) {
        Ok(t) => t.lines().map(str::to_string).collect(),
        Err(_) => vec![],
    }
}

fn read_link(p: &Path) -> std::path::PathBuf {
    std::fs::read_link(p).unwrap()
}

#[test]
fn link_project_symlinks_from_sources_and_registers() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();
    let consumer = Consumer::project(proj.path());

    let mut out = link(
        sd.sd(),
        &consumer,
        &["git-commit".into(), "grilling".into()],
        false,
    )
    .unwrap();
    out.linked.sort();
    assert_eq!(out.linked, vec!["git-commit", "grilling"]);

    let skills = proj.path().join(".agents/skills");
    // Authored links from the Store; vendored from the Cache.
    assert_eq!(
        read_link(&skills.join("git-commit")),
        sd.sd().authored_skill_dir("git-commit")
    );
    assert_eq!(
        read_link(&skills.join("grilling")),
        sd.sd()
            .cache_clone_dir("local/test/skills")
            .join("skills/grilling")
    );

    // Entry link so Claude Code discovers them.
    let entry = proj.path().join(".claude/skills");
    assert_eq!(read_link(&entry), Path::new("../.agents/skills"));

    // Registered.
    let canonical = std::fs::canonicalize(proj.path()).unwrap();
    assert_eq!(registry_lines(sd.sd()), vec![canonical.to_string_lossy()]);
}

#[test]
fn link_global_double_writes_both_trees_without_registering() {
    let sd = setup();
    let home = tempfile::tempdir().unwrap();
    let consumer = Consumer::Global {
        agents: home.path().join(".agents"),
        claude: home.path().join(".claude"),
    };

    link(sd.sd(), &consumer, &["grilling".into()], false).unwrap();

    let src = sd
        .sd()
        .cache_clone_dir("local/test/skills")
        .join("skills/grilling");
    assert_eq!(read_link(&home.path().join(".agents/skills/grilling")), src);
    assert_eq!(read_link(&home.path().join(".claude/skills/grilling")), src);

    // Global consumers are not registered.
    assert!(registry_lines(sd.sd()).is_empty());
}

#[test]
fn link_by_repo_identity_links_all_its_skills() {
    let sd = TempSkilldock::new();
    let repo = GitFixture::init();
    repo.add_skill("a/one", &skill_md("one"));
    repo.add_skill("a/two", &skill_md("two"));
    repo.commit("seed");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/multi".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("a/*".into())],
        },
    )
    .unwrap();

    let proj = tempfile::tempdir().unwrap();
    let mut out = link(
        sd.sd(),
        &Consumer::project(proj.path()),
        &["local/test/multi".into()],
        false,
    )
    .unwrap();
    out.linked.sort();
    assert_eq!(out.linked, vec!["one", "two"]);
}

#[test]
fn ambiguous_name_is_rejected() {
    let sd = TempSkilldock::new();
    author(sd.sd(), "dup").unwrap();
    let repo = GitFixture::init();
    repo.add_skill("s/dup", &skill_md("dup"));
    repo.commit("seed");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/dup".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("s/dup".into())],
        },
    )
    .unwrap();

    let proj = tempfile::tempdir().unwrap();
    let err = link(
        sd.sd(),
        &Consumer::project(proj.path()),
        &["dup".into()],
        false,
    );
    assert!(err.is_err(), "ambiguous name must be rejected");
}

#[test]
fn unlink_last_skill_deregisters_and_tears_down() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();
    let consumer = Consumer::project(proj.path());
    link(sd.sd(), &consumer, &["git-commit".into()], false).unwrap();

    let out = unlink(sd.sd(), &consumer, &["git-commit".into()]).unwrap();
    assert_eq!(out.removed, vec!["git-commit"]);
    assert!(out.deregistered);

    assert!(
        !proj.path().join(".agents/skills").exists(),
        "empty dir removed"
    );
    assert!(
        !proj.path().join(".claude/skills").exists(),
        "entry link removed"
    );
    assert!(registry_lines(sd.sd()).is_empty());
}

#[test]
fn unlink_one_of_many_keeps_registration() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();
    let consumer = Consumer::project(proj.path());
    link(
        sd.sd(),
        &consumer,
        &["git-commit".into(), "grilling".into()],
        false,
    )
    .unwrap();

    let out = unlink(sd.sd(), &consumer, &["grilling".into()]).unwrap();
    assert_eq!(out.removed, vec!["grilling"]);
    assert!(!out.deregistered);
    assert!(proj.path().join(".agents/skills/git-commit").exists());
    assert!(!proj.path().join(".agents/skills/grilling").exists());
    assert_eq!(registry_lines(sd.sd()).len(), 1);
}

#[test]
fn prune_removes_dangling_links() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();
    let consumer = Consumer::project(proj.path());
    link(
        sd.sd(),
        &consumer,
        &["git-commit".into(), "grilling".into()],
        false,
    )
    .unwrap();

    // Break the authored link by deleting its Source.
    std::fs::remove_dir_all(sd.sd().authored_skill_dir("git-commit")).unwrap();

    let out = prune(sd.sd(), &consumer).unwrap();
    assert_eq!(out.pruned, vec!["git-commit"]);
    assert!(!out.deregistered, "grilling still linked");
    assert!(!proj.path().join(".agents/skills/git-commit").exists());
    assert!(proj.path().join(".agents/skills/grilling").exists());
}

#[test]
fn prune_last_dangling_link_deregisters() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();
    let consumer = Consumer::project(proj.path());
    link(sd.sd(), &consumer, &["git-commit".into()], false).unwrap();

    std::fs::remove_dir_all(sd.sd().authored_skill_dir("git-commit")).unwrap();

    let out = prune(sd.sd(), &consumer).unwrap();
    assert_eq!(out.pruned, vec!["git-commit"]);
    assert!(out.deregistered, "pruning the last link deregisters");
    assert!(!proj.path().join(".agents/skills").exists());
    assert!(registry_lines(sd.sd()).is_empty());
}

#[test]
fn global_ops_skip_links_owned_by_another_store() {
    let sd = setup();
    let home = tempfile::tempdir().unwrap();
    let consumer = Consumer::Global {
        agents: home.path().join(".agents"),
        claude: home.path().join(".claude"),
    };

    // A link whose name matches a model skill but points into a foreign store.
    let agents_skills = home.path().join(".agents/skills");
    std::fs::create_dir_all(&agents_skills).unwrap();
    let foreign = home.path().join("foreign-store/grilling");
    std::os::unix::fs::symlink(&foreign, agents_skills.join("grilling")).unwrap();

    // relink must not hijack it toward this skilldock.
    relink(sd.sd(), &consumer).unwrap();
    assert_eq!(read_link(&agents_skills.join("grilling")), foreign);

    // unlink -g must not remove it either.
    let out = unlink(sd.sd(), &consumer, &["grilling".into()]).unwrap();
    assert!(out.removed.is_empty(), "foreign link left alone");
    assert_eq!(read_link(&agents_skills.join("grilling")), foreign);
}

#[test]
fn relink_repoints_a_stale_link_to_the_current_source() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();
    let skills = proj.path().join(".agents/skills");
    std::fs::create_dir_all(&skills).unwrap();

    // A hand-made link pointing at a stale path.
    let stale = proj.path().join("old-store/git-commit");
    std::os::unix::fs::symlink(&stale, skills.join("git-commit")).unwrap();

    let out = relink(sd.sd(), &Consumer::project(proj.path())).unwrap();
    assert_eq!(out.repointed, vec!["git-commit"]);
    assert_eq!(
        read_link(&skills.join("git-commit")),
        sd.sd().authored_skill_dir("git-commit")
    );
}

#[test]
fn relink_leaves_links_unknown_to_the_model() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();
    let skills = proj.path().join(".agents/skills");
    std::fs::create_dir_all(&skills).unwrap();
    let foreign = proj.path().join("foreign");
    std::os::unix::fs::symlink(&foreign, skills.join("stranger")).unwrap();

    let out = relink(sd.sd(), &Consumer::project(proj.path())).unwrap();
    assert!(out.repointed.is_empty());
    assert!(out.unchanged.is_empty());
    assert_eq!(read_link(&skills.join("stranger")), foreign);
}

#[test]
fn register_and_deregister_are_explicit_and_idempotent() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();

    assert!(
        register(sd.sd(), proj.path()).unwrap(),
        "first register adds"
    );
    assert!(
        !register(sd.sd(), proj.path()).unwrap(),
        "second register is a no-op"
    );
    assert_eq!(registry_lines(sd.sd()).len(), 1);

    assert!(deregister(sd.sd(), proj.path()).unwrap(), "removes it");
    assert!(
        !deregister(sd.sd(), proj.path()).unwrap(),
        "second deregister is a no-op"
    );
    assert!(registry_lines(sd.sd()).is_empty());
}
