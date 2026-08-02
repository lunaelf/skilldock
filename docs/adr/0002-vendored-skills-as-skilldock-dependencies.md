# Vendored skills are Skilldock dependencies linked directly into consumers

After collapsing the old npx/external categories into one `vendored` provenance (vs `authored`),
vendored skills are cached in a per-user `~/.skilldock/<host>/<owner>/<repo>` clone pinned by ref
and symlinked **directly** into each Consumer (a project's `.agents/skills/` and the global config
dir). The Store repo holds only authored originals and the manifest — it no longer contains
vendored skill files, and (per ADR-0003) no longer contains the tooling either. Pinning is per source-repo (one clone = one SHA; skills from the
same repo update together) with a per-skill content hash for integrity. Dependency state is split
cargo-style: a hand-edited `skills.toml` (declared repos, refs, subpaths, and the authored
allowlist) and a tool-owned `skills.lock` (resolved SHA + per-skill hash).

**Why:** this is the idiomatic package-manager model for a Rust product — it keeps the repo free of
committed third-party code, and a `skills.toml` + `skills.lock` reproduce everything via
`skills sync`.

## Considered options

- **Pinned committed copies in-repo (P, vendor-into-repo).** Rejected: commits third-party files
  into the repo and abandons the dependency/cache model. The trade taken was a clean, manifest-only
  repo over offline self-containment.
- **Store-as-hub topology (Skilldock → Store → Consumer).** Rejected in favour of Skilldock →
  Consumer directly; the Store stops being the materialisation hub.

## Consequences

- The Store repo is **not** self-contained offline: a fresh machine needs `skills sync` plus live
  upstream repos. Upstream deletion / force-push of a pinned SHA loses the skill unless the
  Skilldock is backed up (the left-pad risk).
- The 25 currently-committed vendored directories are deleted from the repo and moved into the
  Cache; the Store repo shrinks to authored skills + manifest (the tool is split out — ADR-0003).
- A Consumer's `.agents/skills/` now carries symlinks from two roots — the Skilldock (vendored) and
  the Store (authored).
- `links.txt` (the downstream registry of which consumers hold links) is orthogonal upstream/
  downstream state and stays machine-local and gitignored.
