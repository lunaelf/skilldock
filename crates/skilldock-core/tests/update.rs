//! `update` re-resolves declared refs to fresh commits, rewrites the lock
//! (new SHA + re-hashed skills), and re-fetches the Cache.

mod common;

use common::{GitFixture, TempSkilldock};
use skilldock_core::{add, update, AddRequest, Lock, SkillSpec, Source};

/// A `SKILL.md` body with controllable content.
fn body(tag: &str) -> String {
    format!("---\nname: s\ndescription: {tag}\n---\n\n# {tag}\n")
}

#[test]
fn update_moves_a_source_to_a_new_commit_and_rehashes() {
    let sd = TempSkilldock::new();
    let repo = GitFixture::init();
    repo.add_skill("s/one", &body("v1"));
    let sha1 = repo.commit("c1");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/proj".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("s/one".into())],
        },
    )
    .unwrap();
    let before = Lock::read(&sd.sd().lock_path()).unwrap();
    assert_eq!(before.repos[0].resolved, sha1);
    let hash1 = before.repos[0].skills[0].hash.clone();

    // Upstream moves: the skill's content changes.
    repo.add_skill("s/one", &body("v2"));
    let sha2 = repo.commit("c2");
    assert_ne!(sha1, sha2);

    let out = update(sd.sd(), &[]).unwrap();
    assert_eq!(out.repos.len(), 1);
    assert!(out.repos[0].moved);
    assert_eq!(out.repos[0].from.as_deref(), Some(sha1.as_str()));
    assert_eq!(out.repos[0].to, sha2);

    // Lock re-pinned and re-hashed.
    let after = Lock::read(&sd.sd().lock_path()).unwrap();
    assert_eq!(after.repos[0].resolved, sha2);
    assert_ne!(after.repos[0].skills[0].hash, hash1, "content hash changed");

    // Cache reflects the new content.
    let clone = sd.sd().cache_clone_dir("local/test/proj");
    assert_eq!(
        std::fs::read_to_string(clone.join("s/one/SKILL.md")).unwrap(),
        body("v2")
    );
}

#[test]
fn update_scopes_to_named_repos_only() {
    let sd = TempSkilldock::new();

    let a = GitFixture::init();
    a.add_skill("s/x", &body("ax1"));
    a.commit("a1");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/a".into(),
                url: a.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("s/x".into())],
        },
    )
    .unwrap();

    let b = GitFixture::init();
    b.add_skill("s/y", &body("by1"));
    let b1 = b.commit("b1");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/b".into(),
                url: b.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("s/y".into())],
        },
    )
    .unwrap();

    // Both upstreams move forward.
    a.add_skill("s/x", &body("ax2"));
    let a2 = a.commit("a2");
    b.add_skill("s/y", &body("by2"));
    let b2 = b.commit("b2");

    // Update only A.
    let out = update(sd.sd(), &["local/test/a".into()]).unwrap();
    assert_eq!(out.repos.len(), 1);
    assert_eq!(out.repos[0].repo, "local/test/a");

    let lock = Lock::read(&sd.sd().lock_path()).unwrap();
    let ra = lock
        .repos
        .iter()
        .find(|r| r.repo == "local/test/a")
        .unwrap();
    let rb = lock
        .repos
        .iter()
        .find(|r| r.repo == "local/test/b")
        .unwrap();
    assert_eq!(ra.resolved, a2, "A moved to its new commit");
    assert_eq!(rb.resolved, b1, "B untouched");
    assert_ne!(rb.resolved, b2);
}

#[test]
fn update_unknown_repo_errors() {
    let sd = TempSkilldock::new();
    assert!(update(sd.sd(), &["github.com/no/such".into()]).is_err());
}
