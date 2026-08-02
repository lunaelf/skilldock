//! `doctor` cross-checks the dock and classifies each inconsistency. Each test
//! injects one and asserts the finding kind, its severity, and the error gate.

mod common;

use common::{skill_md, GitFixture, TempSkilldock};
use skilldock_core::{
    add, author, doctor, link, register, AddRequest, Consumer, DoctorOptions, FindingKind, Lock,
    Manifest, Report, Severity, SkillSpec, Source,
};

/// A consistent dock: one authored skill + one vendored repo (two skills).
/// Returns the fixture too, kept alive for `--fix` (which re-clones).
fn setup() -> (TempSkilldock, GitFixture) {
    let sd = TempSkilldock::new();
    author(sd.sd(), "git-commit").unwrap();
    let repo = GitFixture::init();
    repo.add_skill("s/one", &skill_md("one"));
    repo.add_skill("s/two", &skill_md("two"));
    repo.commit("seed");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/proj".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![
                SkillSpec::Path("s/one".into()),
                SkillSpec::Path("s/two".into()),
            ],
        },
    )
    .unwrap();
    (sd, repo)
}

fn has(report: &Report, kind: FindingKind) -> bool {
    report.findings.iter().any(|f| f.kind == kind)
}

#[test]
fn healthy_dock_reports_nothing() {
    let (sd, _repo) = setup();
    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(
        report.findings.is_empty(),
        "unexpected: {:?}",
        report.findings
    );
    assert!(!report.has_errors());
}

#[test]
fn declared_but_unlocked_is_an_error() {
    let (sd, _repo) = setup();
    let mut lock = Lock::read(&sd.sd().lock_path()).unwrap();
    lock.remove_repo("local/test/proj");
    lock.write(&sd.sd().lock_path()).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::DeclaredButUnlocked));
    assert!(report.has_errors());
}

#[test]
fn stale_lock_is_an_error() {
    let (sd, _repo) = setup();
    let mut manifest = Manifest::read(&sd.sd().manifest_path()).unwrap();
    manifest.remove_vendored_repo("local/test/proj");
    manifest.write(&sd.sd().manifest_path()).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::StaleLock));
    assert!(report.has_errors());
}

#[test]
fn glob_drift_is_an_error() {
    let sd = TempSkilldock::new();
    let repo = GitFixture::init();
    repo.add_skill("s/one", &skill_md("one"));
    repo.commit("seed");
    add(
        sd.sd(),
        AddRequest {
            source: Source {
                repo: "local/test/glob".into(),
                url: repo.url(),
            },
            git_ref: None,
            skills: vec![SkillSpec::Path("s/*".into())],
        },
    )
    .unwrap();

    // A new skill appears in the working tree (HEAD unchanged) -> the glob now
    // expands to more than the lock records.
    let clone = sd.sd().cache_clone_dir("local/test/glob");
    std::fs::create_dir_all(clone.join("s/three")).unwrap();
    std::fs::write(clone.join("s/three/SKILL.md"), skill_md("three")).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::GlobDrift));
    assert!(report.has_errors());
}

#[test]
fn cache_sha_mismatch_is_an_error() {
    let (sd, _repo) = setup();
    let mut lock = Lock::read(&sd.sd().lock_path()).unwrap();
    lock.repos[0].resolved = "0000000000000000000000000000000000000000".into();
    lock.write(&sd.sd().lock_path()).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::CacheShaMismatch));
    assert!(report.has_errors());
}

#[test]
fn hash_mismatch_only_surfaces_under_verify() {
    let (sd, _repo) = setup();
    // Tamper with a cached skill's content (HEAD unchanged, so SHA still matches).
    let clone = sd.sd().cache_clone_dir("local/test/proj");
    std::fs::write(clone.join("s/one/SKILL.md"), "tampered\n").unwrap();

    let plain = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(
        !has(&plain, FindingKind::HashMismatch),
        "no hashing without --verify"
    );

    let verified = doctor(
        sd.sd(),
        DoctorOptions {
            verify: true,
            ..DoctorOptions::default()
        },
    )
    .unwrap();
    assert!(has(&verified, FindingKind::HashMismatch));
    assert!(verified.has_errors());
}

#[test]
fn store_orphan_is_an_error() {
    let (sd, _repo) = setup();
    let rogue = sd.sd().authored_skill_dir("rogue");
    std::fs::create_dir_all(&rogue).unwrap();
    std::fs::write(rogue.join("SKILL.md"), skill_md("rogue")).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::StoreOrphan));
    assert!(report.has_errors());
}

#[test]
fn missing_authored_dir_is_an_error() {
    let (sd, _repo) = setup();
    std::fs::remove_dir_all(sd.sd().authored_skill_dir("git-commit")).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::MissingAuthoredDir));
    assert!(report.has_errors());
}

#[test]
fn name_collision_is_an_error() {
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

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::NameCollision));
    assert!(report.has_errors());
}

#[test]
fn dangling_link_is_a_warning_and_respects_no_consumers() {
    let sd = TempSkilldock::new();
    let proj = tempfile::tempdir().unwrap();
    let skills = proj.path().join(".agents/skills");
    std::fs::create_dir_all(&skills).unwrap();
    std::os::unix::fs::symlink(proj.path().join("gone"), skills.join("ghost")).unwrap();
    register(sd.sd(), proj.path()).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::DanglingLink));
    assert_eq!(
        report
            .findings
            .iter()
            .find(|f| f.kind == FindingKind::DanglingLink)
            .unwrap()
            .severity(),
        Severity::Warning
    );
    assert!(!report.has_errors(), "a dangling link alone doesn't gate");

    // --no-consumers skips it.
    let scoped = doctor(
        sd.sd(),
        DoctorOptions {
            consumers: false,
            ..DoctorOptions::default()
        },
    )
    .unwrap();
    assert!(!has(&scoped, FindingKind::DanglingLink));
}

#[test]
fn removed_skill_link_is_a_warning() {
    let sd = TempSkilldock::new();
    let proj = tempfile::tempdir().unwrap();
    let skills = proj.path().join(".agents/skills");
    std::fs::create_dir_all(&skills).unwrap();
    // A resolving link whose name isn't in the (empty) model.
    let target = proj.path().join("elsewhere");
    std::fs::create_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&target, skills.join("gone")).unwrap();
    register(sd.sd(), proj.path()).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::RemovedSkillLink));
    assert!(!report.has_errors());
}

#[test]
fn missing_consumer_dir_is_a_warning() {
    let sd = TempSkilldock::new();
    let proj = tempfile::tempdir().unwrap();
    register(sd.sd(), proj.path()).unwrap();
    std::fs::remove_dir_all(proj.path()).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::MissingConsumerDir));
    assert!(!report.has_errors());
}

#[test]
fn empty_registration_is_a_warning() {
    let sd = TempSkilldock::new();
    let proj = tempfile::tempdir().unwrap();
    register(sd.sd(), proj.path()).unwrap(); // registered, but no links

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::EmptyRegistration));
    assert!(!report.has_errors());
}

#[test]
fn orphan_clone_is_a_warning() {
    let sd = TempSkilldock::new();
    // A clone-shaped dir under cache/ that no lock entry references.
    let orphan = sd.sd().cache_clone_dir("local/test/orphan");
    std::fs::create_dir_all(orphan.join(".git")).unwrap();

    let report = doctor(sd.sd(), DoctorOptions::default()).unwrap();
    assert!(has(&report, FindingKind::OrphanClone));
    assert!(!report.has_errors());
}

#[test]
fn fix_reconciles_a_wiped_cache() {
    let (sd, _repo) = setup();
    std::fs::remove_dir_all(sd.sd().cache()).unwrap();
    assert!(has(
        &doctor(sd.sd(), DoctorOptions::default()).unwrap(),
        FindingKind::MissingClone
    ));

    let fixed = doctor(
        sd.sd(),
        DoctorOptions {
            fix: true,
            ..DoctorOptions::default()
        },
    )
    .unwrap();
    assert!(
        !has(&fixed, FindingKind::MissingClone),
        "sync restored the clone"
    );
    assert!(!fixed.has_errors());
}

#[test]
fn fix_prunes_a_dangling_consumer_link() {
    let (sd, _repo) = setup();
    let proj = tempfile::tempdir().unwrap();
    let consumer = Consumer::project(proj.path());
    link(sd.sd(), &consumer, &["git-commit".into()], false).unwrap();
    // Break the link by removing its Source.
    std::fs::remove_dir_all(sd.sd().authored_skill_dir("git-commit")).unwrap();
    // Also drop it from the authored list so it isn't a MissingAuthoredDir error.
    let mut manifest = Manifest::read(&sd.sd().manifest_path()).unwrap();
    manifest.authored.clear();
    manifest.write(&sd.sd().manifest_path()).unwrap();

    let fixed = doctor(
        sd.sd(),
        DoctorOptions {
            fix: true,
            ..DoctorOptions::default()
        },
    )
    .unwrap();
    assert!(!has(&fixed, FindingKind::DanglingLink), "prune removed it");
    assert!(!proj.path().join(".agents/skills/git-commit").exists());
}
