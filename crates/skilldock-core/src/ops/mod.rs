//! The public operation API — the one seam the whole product is tested at
//! (per the PRD's testing decisions). Each operation acts on a real (temp under
//! test) skilldock and produces observable results on the filesystem and manifests.

pub mod add;
pub mod author;
pub mod list;
pub mod sync;
