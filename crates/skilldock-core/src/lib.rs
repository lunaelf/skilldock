//! `skilldock-core` — the single source of truth for all skilldock operations.
//!
//! The CLI (`skilldock`/`sd`) and the Tauri GUI are thin adapters over this
//! library; all behavior and its tests live here (ADR-0001). Operations take an
//! explicit [`Skilldock`] rather than reading the environment, so they run
//! against a throwaway root under test.

mod cache;
mod config;
mod consumer;
mod error;
mod expand;
mod git;
mod glob;
mod hash;
mod linkfs;
mod linking;
mod lock;
mod manifest;
mod ops;
mod registry;
mod resolve;
mod skilldock;
mod source;
mod tomlio;
mod vendored;

pub use config::Config;
pub use consumer::Consumer;
pub use error::{Error, Result};
pub use glob::is_glob;
pub use lock::{Lock, LockRepo, LockSkill};
pub use manifest::{Manifest, SkillSpec, VendoredRepo};
pub use resolve::{linkable, resolve_inputs, Provenance, ResolvedLink};
pub use skilldock::{Skilldock, HOME_ENV};
pub use source::{parse_source, Source};

pub use ops::add::{add, AddOutcome, AddRequest};
pub use ops::author::{author, AuthorOutcome};
pub use ops::doctor::{doctor, DoctorOptions, Finding, FindingKind, Report, Severity};
pub use ops::init::{init, InitOutcome};
pub use ops::link::{link, LinkOutcome};
pub use ops::list::{list, AuthoredSkill, Listing, VendoredSkill};
pub use ops::migrate::{migrate, MigrateOptions, MigrateOutcome, SkillReport, SkillStatus};
pub use ops::prune::{prune, prune_all, PruneOutcome};
pub use ops::register::{deregister, register};
pub use ops::relink::{relink, relink_all, RelinkOutcome};
pub use ops::remove::{remove, RemoveOutcome};
pub use ops::sync::{sync, SyncOutcome};
pub use ops::unlink::{unlink, UnlinkOutcome};
pub use ops::update::{update, RepoUpdate, UpdateOutcome};
