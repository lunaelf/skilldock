# Rewrite the product into Rust; retire Bash

The repo is currently a deliberately zero-build POSIX Bash 3.2 toolset (the scripts are the source
of truth) plus a python-stdlib web UI serving static HTML. We are rewriting the whole thing into a
Rust workspace: a `skills-core` library holding all store logic, exposed as both a `skills-cli`
binary (replacing every Bash script) and a Tauri GUI (`skills-gui`). The Bash scripts and the
python UI server are retired.

**Why:** the Rust codebase is itself the goal — one typed, testable implementation shared by CLI
and GUI. The Rust-toolchain tax is accepted as the point, not a cost.

## Considered options

- **`--app=` one-liner / thin Tauri shell over the existing scripts (4a).** Rejected: it would
  have satisfied the original "native window" ask, but that was never the real goal.
- **Port only mutation logic into Rust, keep Bash as the CLI/CI (4b).** Rejected: the Bash scripts
  can't be deleted (they are also the CLI, the pre-commit gate, and CI), so this duplicates store
  logic across Rust and Bash.

## Consequences

- Rust/Cargo become a prerequisite for working on the repo (previously nothing was).
- The entire convention layer is rewritten: `test.sh` → `cargo test`, `check.sh`, the pre-commit
  hook, `gen-packages`, and the `CLAUDE.md` contract sections.
- Distributing the GUI needs macOS signing/notarization (or ad-hoc signing for personal use).
- The macOS Bash 3.2 footguns documented in `CLAUDE.md` (empty-array expansion, `for d in */`
  status, IFS tab-splitting, no associative arrays) disappear with the language.
