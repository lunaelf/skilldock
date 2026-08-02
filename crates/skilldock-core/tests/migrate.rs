//! `migrate` converts the Bash-era three-manifest repo into the new dock:
//! synthesized toml/lock/Cache/Store, a re-pin-to-HEAD diff report, and the
//! safety rails (dirty-tree refusal, `--dry-run` no-op, manifest backup, and a
//! doctor-gated destructive cleanup).

mod common;

use common::{skill_md, GitFixture, TempSkilldock};
use skilldock_core::{migrate, Manifest, MigrateOptions, SkillSpec, SkillStatus};

/// The canonical identity `parse_source` derives from a fixture's `file://` URL:
/// the path with its leading slash trimmed.
fn identity_of(fx: &GitFixture) -> String {
    fx.path()
        .to_string_lossy()
        .trim_start_matches('/')
        .to_string()
}

/// A vendored Source with two skills under `skills/eng/`.
fn source_two_skills() -> GitFixture {
    let fx = GitFixture::init();
    fx.add_skill("skills/eng/one", "one-at-head");
    fx.add_skill("skills/eng/two", "two-shared");
    fx.commit("seed source A");
    fx
}

/// An external Source with a single top-level skill dir.
fn source_external() -> GitFixture {
    let fx = GitFixture::init();
    fx.add_skill("hv-analysis", &skill_md("hv-analysis"));
    fx.commit("seed external B");
    fx
}

/// A Source whose skill is the repo root (`SKILL.md` at the top).
fn source_repo_root() -> GitFixture {
    let fx = GitFixture::init();
    fx.add_skill(".", &skill_md("humanizer"));
    fx.commit("seed repo-root C");
    fx
}

/// Build a fixture old repo (a committed git tree) with all three manifests, an
/// authored original, and committed vendored copies for the diff comparison.
fn old_repo(a: &GitFixture, b: &GitFixture, c: &GitFixture) -> GitFixture {
    let repo = GitFixture::init();

    let skills_lock = format!(
        r#"{{
  "version": 1,
  "skills": {{
    "one":       {{ "source": "{a}", "sourceType": "github", "skillPath": "skills/eng/one/SKILL.md" }},
    "two":       {{ "source": "{a}", "sourceType": "github", "skillPath": "skills/eng/two/SKILL.md" }},
    "humanizer": {{ "source": "{c}", "sourceType": "github", "skillPath": "SKILL.md" }}
  }}
}}"#,
        a = a.url(),
        c = c.url(),
    );
    let external = format!(
        r#"{{
  "version": 1,
  "skills": {{
    "hv-analysis": {{ "repo": "{b}", "ref": "", "skillPath": "hv-analysis" }}
  }}
}}"#,
        b = b.url(),
    );

    repo.write_file("skills-lock.json", &skills_lock);
    repo.write_file("external.json", &external);
    repo.write_file("authored.txt", "# my own skills\ngit-commit\n");
    // Authored original.
    repo.write_file(
        ".agents/skills/git-commit/SKILL.md",
        &skill_md("git-commit"),
    );
    // Committed vendored copies: `one` diverges from HEAD, `two` matches it.
    repo.add_skill(".agents/skills/one", "one-committed-old");
    repo.add_skill(".agents/skills/two", "two-shared");
    repo.commit("seed old repo");
    repo
}

#[test]
fn migrate_produces_toml_lock_cache_and_store() {
    let a = source_two_skills();
    let b = source_external();
    let c = source_repo_root();
    let old = old_repo(&a, &b, &c);
    let (ida, idb, idc) = (identity_of(&a), identity_of(&b), identity_of(&c));

    let sd = TempSkilldock::new();
    let outcome = migrate(sd.sd(), old.path(), MigrateOptions::default()).unwrap();

    // --- Store: manifest, lock, authored original, git repo, gate. ---
    let manifest = Manifest::read(&sd.sd().manifest_path()).unwrap();
    assert_eq!(manifest.authored, vec!["git-commit"]);

    let mut ids: Vec<&str> = manifest.vendored.iter().map(|v| v.repo.as_str()).collect();
    ids.sort();
    let mut expected = vec![ida.as_str(), idb.as_str(), idc.as_str()];
    expected.sort();
    assert_eq!(ids, expected, "all three sources declared");

    // Source A: two bare-path skills. Source C: a repo-root rename.
    let va = manifest.vendored_repo(&ida).unwrap();
    assert_eq!(
        va.skills,
        vec![
            SkillSpec::Path("skills/eng/one".into()),
            SkillSpec::Path("skills/eng/two".into()),
        ]
    );
    let vc = manifest.vendored_repo(&idc).unwrap();
    assert_eq!(
        vc.skills,
        vec![SkillSpec::Named {
            name: "humanizer".into(),
            path: ".".into(),
        }]
    );

    // Lock: each repo pinned to the fixture HEAD, skills expanded + hashed.
    let lock = outcome.lock.clone();
    let la = lock.repos.iter().find(|r| r.repo == ida).unwrap();
    assert_eq!(la.resolved, a.head());
    let names: Vec<&str> = la.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["one", "two"]);
    assert!(la.skills.iter().all(|s| s.hash.starts_with("sha256:")));
    let lc = lock.repos.iter().find(|r| r.repo == idc).unwrap();
    assert_eq!(lc.skills[0].path, ".", "repo-root skill locks path '.'");

    // Cache: clones checked out at HEAD.
    assert!(sd
        .sd()
        .cache_clone_dir(&ida)
        .join("skills/eng/one/SKILL.md")
        .is_file());
    assert!(sd
        .sd()
        .cache_clone_dir(&idb)
        .join("hv-analysis/SKILL.md")
        .is_file());

    // Store on disk.
    assert!(sd.sd().manifest_path().is_file());
    assert!(sd.sd().lock_path().is_file());
    assert!(sd
        .sd()
        .authored_skill_dir("git-commit")
        .join("SKILL.md")
        .is_file());
    assert!(sd.sd().store().join(".git").is_dir(), "Store is a git repo");
    let hook = sd.sd().store().join(".git/hooks/pre-commit");
    assert!(hook.is_file(), "pre-commit gate installed");
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&hook).unwrap().permissions().mode();
    assert!(mode & 0o111 != 0, "hook executable");

    // Doctor verified the freshly built dock with no errors.
    let report = outcome.doctor.expect("doctor ran");
    assert!(
        !report.has_errors(),
        "built dock is doctor-clean: {:?}",
        report.findings
    );
}

#[test]
fn migrate_reports_which_skills_moved() {
    let a = source_two_skills();
    let b = source_external();
    let c = source_repo_root();
    let old = old_repo(&a, &b, &c);

    let sd = TempSkilldock::new();
    let outcome = migrate(sd.sd(), old.path(), MigrateOptions::default()).unwrap();

    let status = |name: &str| {
        outcome
            .skills
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("no report for {name}"))
            .status
    };
    // `one` diverged from its committed copy; `two` is identical.
    assert_eq!(status("one"), SkillStatus::Moved);
    assert_eq!(status("two"), SkillStatus::Unchanged);
    // No committed copy existed for these → Added.
    assert_eq!(status("hv-analysis"), SkillStatus::Added);
    assert_eq!(status("humanizer"), SkillStatus::Added);
}

#[test]
fn migrate_drops_skills_gone_from_head() {
    let a = GitFixture::init();
    a.add_skill("skills/present", &skill_md("present"));
    a.commit("seed");
    let old = GitFixture::init();
    let skills_lock = format!(
        r#"{{ "version": 1, "skills": {{
            "present": {{ "source": "{a}", "skillPath": "skills/present/SKILL.md" }},
            "gone":    {{ "source": "{a}", "skillPath": "skills/gone/SKILL.md" }}
        }} }}"#,
        a = a.url(),
    );
    old.write_file("skills-lock.json", &skills_lock);
    old.commit("seed old");

    let sd = TempSkilldock::new();
    let outcome = migrate(sd.sd(), old.path(), MigrateOptions::default()).unwrap();

    let gone = outcome.skills.iter().find(|s| s.name == "gone").unwrap();
    assert_eq!(gone.status, SkillStatus::Dropped);

    // Dropped skill is absent from both the toml and the lock, so doctor stays clean.
    let id = identity_of(&a);
    let manifest = Manifest::read(&sd.sd().manifest_path()).unwrap();
    let v = manifest.vendored_repo(&id).unwrap();
    assert_eq!(v.skills, vec![SkillSpec::Path("skills/present".into())]);
    let repo = outcome.lock.repos.iter().find(|r| r.repo == id).unwrap();
    assert_eq!(repo.skills.len(), 1);
    assert!(!outcome.doctor.unwrap().has_errors());
}

#[test]
fn dry_run_changes_nothing_and_is_verifiable() {
    let a = source_two_skills();
    let b = source_external();
    let c = source_repo_root();
    let old = old_repo(&a, &b, &c);

    let sd = TempSkilldock::new();
    let outcome = migrate(
        sd.sd(),
        old.path(),
        MigrateOptions {
            dry_run: true,
            cleanup: false,
        },
    )
    .unwrap();

    // The plan is computed…
    assert!(outcome.dry_run);
    assert_eq!(outcome.manifest.vendored.len(), 3);
    assert!(!outcome.skills.is_empty());
    assert!(outcome.doctor.is_none());
    assert!(outcome.backup.is_none());

    // …but nothing was written to the Store, and the real Cache stays empty
    // (dry-run resolves against a throwaway cache).
    assert!(!sd.sd().manifest_path().exists(), "no toml written");
    assert!(!sd.sd().lock_path().exists(), "no lock written");
    assert!(
        !sd.sd().store().join(".git").exists(),
        "Store not git-init'd"
    );
    assert!(
        std::fs::read_dir(sd.sd().cache()).unwrap().next().is_none(),
        "real Cache untouched by dry-run"
    );
}

#[test]
fn dirty_old_repo_aborts_a_real_migration() {
    let a = source_two_skills();
    let b = source_external();
    let c = source_repo_root();
    let old = old_repo(&a, &b, &c);
    // Uncommitted change → dirty tree.
    old.write_file("scratch.txt", "wip");

    let sd = TempSkilldock::new();
    let err = migrate(sd.sd(), old.path(), MigrateOptions::default());
    assert!(err.is_err(), "dirty tree must abort");
    assert!(!sd.sd().manifest_path().exists(), "nothing built on abort");
}

#[test]
fn migrate_backs_up_the_old_manifests() {
    let a = source_two_skills();
    let b = source_external();
    let c = source_repo_root();
    let old = old_repo(&a, &b, &c);

    let sd = TempSkilldock::new();
    let outcome = migrate(sd.sd(), old.path(), MigrateOptions::default()).unwrap();

    let backup = outcome.backup.expect("backup written");
    assert!(backup.join("skills-lock.json").is_file());
    assert!(backup.join("authored.txt").is_file());
    assert!(backup.join("external.json").is_file());
    // Non-destructive by default: the originals remain and cleanup didn't run.
    assert!(old.path().join("skills-lock.json").is_file());
    assert!(!outcome.cleaned);
    // The backup lives under the dock, so without --cleanup the old repo's git
    // tree is untouched.
    assert!(backup.starts_with(sd.sd().root()));
    assert!(
        common::git_status(old.path()).trim().is_empty(),
        "old repo tree stays clean without --cleanup"
    );
}

#[test]
fn cleanup_removes_old_manifests_after_doctor_passes() {
    let a = source_two_skills();
    let b = source_external();
    let c = source_repo_root();
    let old = old_repo(&a, &b, &c);

    let sd = TempSkilldock::new();
    let outcome = migrate(
        sd.sd(),
        old.path(),
        MigrateOptions {
            dry_run: false,
            cleanup: true,
        },
    )
    .unwrap();

    assert!(!outcome.doctor.unwrap().has_errors());
    assert!(outcome.cleaned);
    for name in ["skills-lock.json", "authored.txt", "external.json"] {
        assert!(!old.path().join(name).exists(), "{name} removed");
    }
    // The backup still holds the originals.
    let backup = outcome.backup.expect("backup written");
    assert!(backup.join("skills-lock.json").is_file());
}

#[test]
fn cleanup_is_skipped_when_doctor_finds_errors() {
    // Two different sources both provide a skill named `dup` → a name collision,
    // which doctor treats as an error. Cleanup must not run.
    let a = GitFixture::init();
    a.add_skill("s/dup", &skill_md("dup"));
    a.commit("seed a");
    let b = GitFixture::init();
    b.add_skill("dup", &skill_md("dup"));
    b.commit("seed b");

    let old = GitFixture::init();
    old.write_file(
        "skills-lock.json",
        &format!(
            r#"{{ "version": 1, "skills": {{ "dup": {{ "source": "{a}", "skillPath": "s/dup/SKILL.md" }} }} }}"#,
            a = a.url()
        ),
    );
    old.write_file(
        "external.json",
        &format!(
            r#"{{ "version": 1, "skills": {{ "dup": {{ "repo": "{b}", "ref": "", "skillPath": "dup" }} }} }}"#,
            b = b.url()
        ),
    );
    old.commit("seed old");

    let sd = TempSkilldock::new();
    let outcome = migrate(
        sd.sd(),
        old.path(),
        MigrateOptions {
            dry_run: false,
            cleanup: true,
        },
    )
    .unwrap();

    assert!(
        outcome.doctor.unwrap().has_errors(),
        "collision is an error"
    );
    assert!(!outcome.cleaned, "cleanup gated behind a clean doctor");
    assert!(
        old.path().join("skills-lock.json").is_file(),
        "old manifests preserved when verification fails"
    );
}

#[test]
fn migrate_refuses_an_already_populated_dock() {
    let a = source_two_skills();
    let b = source_external();
    let c = source_repo_root();
    let old = old_repo(&a, &b, &c);

    let sd = TempSkilldock::new();
    migrate(sd.sd(), old.path(), MigrateOptions::default()).unwrap();
    // A second migrate must not clobber the built Store.
    assert!(migrate(sd.sd(), old.path(), MigrateOptions::default()).is_err());
}
