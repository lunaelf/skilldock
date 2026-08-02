//! `init` reconstructs a whole dock from nothing but the tool + a data-repo URL:
//! clone the Store, write config, install the gate, and sync the Cache.

mod common;

use common::{skill_md, GitFixture, TempSkilldock};
use skilldock_core::{init, Config};

/// Build a data repo (the Store contents) declaring one vendored skill from
/// `vendored`, plus one authored skill. Returns the data-repo fixture.
fn data_repo(vendored: &GitFixture, vendored_sha: &str) -> GitFixture {
    let toml = "authored = [\"mine\"]\n\n\
        [[vendored]]\n\
        repo = \"local/test/vendored\"\n\
        skills = [\"s/one\"]\n";
    let lock = format!(
        "[[repo]]\n\
         repo = \"local/test/vendored\"\n\
         url = \"{url}\"\n\
         resolved = \"{sha}\"\n\n\
         [[repo.skill]]\n\
         name = \"one\"\n\
         path = \"s/one\"\n\
         hash = \"sha256:placeholder\"\n",
        url = vendored.url(),
        sha = vendored_sha,
    );

    let repo = GitFixture::init();
    repo.write_file("skilldock.toml", toml);
    repo.write_file("skilldock.lock", &lock);
    repo.write_file("skills/mine/SKILL.md", &skill_md("mine"));
    repo.commit("seed data repo");
    repo
}

#[test]
fn init_reconstructs_store_config_and_cache() {
    // A vendored Source the data repo's lock points at.
    let vendored = GitFixture::init();
    vendored.add_skill("s/one", &skill_md("one"));
    let sha = vendored.commit("seed vendored");

    let data = data_repo(&vendored, &sha);

    // Empty dock: nothing but the root exists.
    let sd = TempSkilldock::empty();
    let outcome = init(sd.sd(), &data.url()).unwrap();

    // Store: cloned data repo with manifest + authored skill.
    assert!(sd.sd().manifest_path().is_file(), "skilldock.toml cloned");
    assert!(sd.sd().lock_path().is_file(), "skilldock.lock cloned");
    assert!(sd
        .sd()
        .authored_skill_dir("mine")
        .join("SKILL.md")
        .is_file());

    // Config: records the data-repo remote.
    let config = Config::read(&sd.sd().config_path()).unwrap();
    assert_eq!(config.data_repo.as_deref(), Some(data.url().as_str()));

    // Cache: sync populated the vendored clone from the lock.
    assert_eq!(
        outcome.synced.cloned,
        vec!["local/test/vendored".to_string()]
    );
    let clone = sd.sd().cache_clone_dir("local/test/vendored");
    assert!(
        clone.join("s/one/SKILL.md").is_file(),
        "vendored skill synced"
    );

    assert_eq!(outcome.store, sd.sd().store());
}

#[test]
fn init_installs_an_executable_pre_commit_gate() {
    let vendored = GitFixture::init();
    vendored.add_skill("s/one", &skill_md("one"));
    let sha = vendored.commit("seed");
    let data = data_repo(&vendored, &sha);

    let sd = TempSkilldock::empty();
    init(sd.sd(), &data.url()).unwrap();

    let hook = sd.sd().store().join(".git/hooks/pre-commit");
    assert!(hook.is_file(), "pre-commit gate installed");
    let body = std::fs::read_to_string(&hook).unwrap();
    assert!(
        body.contains("skilldock doctor"),
        "gate runs doctor:\n{body}"
    );

    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&hook).unwrap().permissions().mode();
    assert!(mode & 0o111 != 0, "hook is executable (mode {mode:o})");
}

#[test]
fn init_refuses_an_already_initialized_dock() {
    let vendored = GitFixture::init();
    vendored.add_skill("s/one", &skill_md("one"));
    let sha = vendored.commit("seed");
    let data = data_repo(&vendored, &sha);

    let sd = TempSkilldock::empty();
    init(sd.sd(), &data.url()).unwrap();
    // Second init must not clobber the existing Store.
    assert!(init(sd.sd(), &data.url()).is_err());
}

#[test]
fn init_refuses_a_non_checkout_store() {
    let vendored = GitFixture::init();
    vendored.add_skill("s/one", &skill_md("one"));
    let sha = vendored.commit("seed");
    let data = data_repo(&vendored, &sha);

    // A dock whose store/ exists but isn't a git checkout.
    let sd = TempSkilldock::new(); // ensure_layout creates store/skills
    assert!(
        init(sd.sd(), &data.url()).is_err(),
        "must not clone over a non-checkout Store"
    );
}
