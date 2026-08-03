//! Consumer linking: link/unlink/prune/relink/register against a temp Consumer,
//! asserting on the resulting symlinks and `links.txt`.

mod common;

use std::path::Path;

use common::{skill_md, GitFixture, TempSkilldock};
use skilldock_core::{
    add, author, deregister, link, link_status, prune, prune_all, register, relink, relink_all,
    unlink, AddRequest, Consumer, LinkState, SkillSpec, Source,
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
fn relink_all_repoints_every_registered_project() {
    let sd = setup();

    // Two projects, each with a hand-made stale link the model still knows.
    let projects: Vec<_> = (0..2).map(|_| tempfile::tempdir().unwrap()).collect();
    for proj in &projects {
        let skills = proj.path().join(".agents/skills");
        std::fs::create_dir_all(&skills).unwrap();
        let stale = proj.path().join("old-store/git-commit");
        std::os::unix::fs::symlink(&stale, skills.join("git-commit")).unwrap();
        register(sd.sd(), proj.path()).unwrap();
    }

    let results = relink_all(sd.sd()).unwrap();
    assert_eq!(results.len(), 2, "both registered projects processed");
    for (_, out) in &results {
        assert_eq!(out.repointed, vec!["git-commit"]);
    }
    for proj in &projects {
        assert_eq!(
            read_link(&proj.path().join(".agents/skills/git-commit")),
            sd.sd().authored_skill_dir("git-commit")
        );
    }
}

#[test]
fn prune_all_prunes_and_deregisters_across_projects() {
    let sd = setup();

    // Project A keeps a live link; project B has only a dangling one.
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    link(
        sd.sd(),
        &Consumer::project(a.path()),
        &["git-commit".into(), "grilling".into()],
        false,
    )
    .unwrap();
    link(
        sd.sd(),
        &Consumer::project(b.path()),
        &["grilling".into()],
        false,
    )
    .unwrap();

    // Break every `grilling` link by dropping the vendored Source clone.
    std::fs::remove_dir_all(sd.sd().cache_clone_dir("local/test/skills")).unwrap();

    let mut results = prune_all(sd.sd()).unwrap();
    results.sort_by(|x, y| x.0.cmp(&y.0));
    assert_eq!(results.len(), 2);

    // A: grilling pruned, git-commit survives → still registered.
    let a_canon = std::fs::canonicalize(a.path()).unwrap();
    let (_, a_out) = results.iter().find(|(p, _)| *p == a_canon).unwrap();
    assert_eq!(a_out.pruned, vec!["grilling"]);
    assert!(!a_out.deregistered);
    assert!(a.path().join(".agents/skills/git-commit").exists());

    // B: its only link was dangling → pruned and deregistered.
    let b_canon = std::fs::canonicalize(b.path()).unwrap();
    let (_, b_out) = results.iter().find(|(p, _)| *p == b_canon).unwrap();
    assert_eq!(b_out.pruned, vec!["grilling"]);
    assert!(b_out.deregistered);

    // Only project A remains registered.
    assert_eq!(registry_lines(sd.sd()), vec![a_canon.to_string_lossy()]);
}

#[test]
fn link_status_reports_linked_unlinked_and_dangling() {
    let sd = setup();
    // A third dock skill we never link, to observe the Unlinked state.
    author(sd.sd(), "docs").unwrap();
    let proj = tempfile::tempdir().unwrap();
    let consumer = Consumer::project(proj.path());

    link(
        sd.sd(),
        &consumer,
        &["git-commit".into(), "grilling".into()],
        false,
    )
    .unwrap();

    // Break the vendored `grilling` link by dropping its Source clone.
    std::fs::remove_dir_all(sd.sd().cache_clone_dir("local/test/skills")).unwrap();

    let status = link_status(sd.sd(), &consumer).unwrap();
    // Every dock skill, once each, name-sorted.
    let names: Vec<_> = status.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["docs", "git-commit", "grilling"]);

    let by_name: std::collections::BTreeMap<_, _> =
        status.iter().map(|s| (s.name.as_str(), s.state)).collect();
    assert_eq!(by_name["git-commit"], LinkState::Linked); // authored Source intact
    assert_eq!(by_name["grilling"], LinkState::Dangling); // Source removed
    assert_eq!(by_name["docs"], LinkState::Unlinked); // never linked
}

#[test]
fn link_status_ignores_global_links_owned_by_another_store() {
    let sd = setup();
    let home = tempfile::tempdir().unwrap();
    let consumer = Consumer::Global {
        agents: home.path().join(".agents"),
        claude: home.path().join(".claude"),
    };

    // A broken link named like a dock skill but pointing into a foreign store —
    // the shape of a stale, pre-migration global link.
    let agents_skills = home.path().join(".agents/skills");
    std::fs::create_dir_all(&agents_skills).unwrap();
    let foreign = home.path().join("foreign-store/grilling");
    std::os::unix::fs::symlink(&foreign, agents_skills.join("grilling")).unwrap();

    let status = link_status(sd.sd(), &consumer).unwrap();
    let by_name: std::collections::BTreeMap<_, _> =
        status.iter().map(|s| (s.name.as_str(), s.state)).collect();
    // Not this dock's link → Unlinked, not Dangling; consistent with unlink/prune/
    // relink, which leave foreign global links alone (a `link --force` reclaims it).
    assert_eq!(by_name["grilling"], LinkState::Unlinked);
}

#[test]
fn link_status_of_a_fresh_consumer_is_all_unlinked() {
    let sd = setup();
    let proj = tempfile::tempdir().unwrap();
    let status = link_status(sd.sd(), &Consumer::project(proj.path())).unwrap();
    assert!(!status.is_empty(), "dock skills are listed");
    assert!(status.iter().all(|s| s.state == LinkState::Unlinked));
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
