//! `sync` reconstructs the Cache to exactly match the lock: a wiped Cache comes
//! back with the same clones checked out at the same SHAs and identical content.

mod common;

use std::process::Command;

use common::{git_head, skill_md, GitFixture, TempSkilldock};
use skilldock_core::{add, sync, AddRequest, Lock, SkillSpec, Source};

#[test]
fn sync_rebuilds_a_wiped_cache_from_the_lock() {
    let sd = TempSkilldock::new();
    let repo = GitFixture::init();
    repo.add_skill("skills/grilling", &skill_md("grilling"));
    repo.add_skill("skills/prototype", &skill_md("prototype"));
    let sha = repo.commit("seed");

    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/skills".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("skills/*".into())],
        },
    )
    .unwrap();

    // Wipe the entire Cache — the left-pad scenario, or a fresh machine.
    std::fs::remove_dir_all(sd.sd().cache()).unwrap();
    let clone = sd.sd().cache_clone_dir("local/test/skills");
    assert!(!clone.exists());

    let outcome = sync(sd.sd()).unwrap();
    assert_eq!(outcome.cloned, vec!["local/test/skills".to_string()]);

    // Restored: clone present, checked out at the locked SHA, content identical.
    assert!(clone.join(".git").is_dir());
    assert_eq!(git_head(&clone), sha, "checked out to the locked SHA");
    assert_eq!(
        std::fs::read_to_string(clone.join("skills/grilling/SKILL.md")).unwrap(),
        skill_md("grilling")
    );
    assert!(clone.join("skills/prototype/SKILL.md").is_file());
}

#[test]
fn sync_checks_out_an_existing_clone_to_the_locked_sha() {
    let sd = TempSkilldock::new();
    let repo = GitFixture::init();
    repo.add_skill("s/one", &skill_md("one"));
    let pinned = repo.commit("first");

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

    // The clone drifts to a newer commit behind sync's back.
    let clone = sd.sd().cache_clone_dir("local/test/proj");
    std::fs::write(clone.join("drift.txt"), "x").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&clone)
        .status()
        .unwrap();
    Command::new("git")
        .args([
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "drift",
        ])
        .current_dir(&clone)
        .status()
        .unwrap();
    assert_ne!(git_head(&clone), pinned);

    sync(sd.sd()).unwrap();

    // Back on the locked SHA.
    let lock = Lock::read(&sd.sd().lock_path()).unwrap();
    assert_eq!(lock.repos[0].resolved, pinned);
    assert_eq!(git_head(&clone), pinned);
}
