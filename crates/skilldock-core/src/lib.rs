//! `skilldock-core` — the single source of truth for all skilldock operations.
//!
//! The CLI (`skilldock`/`sd`) and the Tauri GUI are thin adapters over this
//! library; all behavior and its tests live here (ADR-0001). Operations take an
//! explicit [`Skilldock`] rather than reading the environment, so they run
//! against a throwaway root under test.

mod error;
mod glob;
mod lock;
mod manifest;
mod ops;
mod skilldock;
mod tomlio;

pub use error::{Error, Result};
pub use glob::is_glob;
pub use lock::{Lock, LockRepo, LockSkill};
pub use manifest::{Manifest, SkillSpec, VendoredRepo};
pub use skilldock::{Skilldock, HOME_ENV};

pub use ops::author::{author, AuthorOutcome};
pub use ops::list::{list, AuthoredSkill, Listing, VendoredSkill};
