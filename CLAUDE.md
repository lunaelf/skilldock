# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

The Rust workspace for **`skilldock`** — a personal tool that manages Agent Skills. It keeps the
originals of skills you wrote, caches the ones you pull from other people's repos, and links both
into the projects that use them (by **symlink**, not copy) so an update to an original flows through
everywhere without duplication.

This repo holds **only the tool** (a `cargo install`ed binary). The data it operates on lives
elsewhere:

- the **dock** at `~/.skilldock` (override with `SKILLDOCK_HOME`) — the Store checkout, the Cache of
  vendored clones, and `config.toml`;
- the **Store** = the data repo (`github.com/lunaelf/skills`) checked out at `~/.skilldock/store`:
  authored skill originals + the manifest (`skilldock.toml` / `skilldock.lock`).

Read `CONTEXT.md` first — it is the domain glossary (Skill, Provenance, Vendored/Authored, Skilldock,
Store, Cache, Source, Consumer, Link) and every term here is used in that exact sense. The design is
recorded in `docs/adr/0001–0004`; the product spec is GitHub issue **#1**.

> History: this repo began as Bash tooling with a three-manifest model (`skills-lock.json` /
> `authored.txt` / `external.json`) and vendored skill dirs under `.agents/skills/`. That was
> rewritten into Rust (ADR-0001) and cut over via `skilldock migrate`. Don't reintroduce the Bash
> tooling or a Node-at-runtime dependency (the GUI's build-time Vite is the only exception).

## The model (one paragraph)

A skill is a directory containing `SKILL.md`. Its **provenance** is either **authored** (original in
the Store, edited directly) or **vendored** (original in someone else's repo, cloned into the Cache
pinned to a ref). `skilldock.toml` declares what the dock should contain; `skilldock.lock` records
the exact resolved commit + content hash per vendored skill so `sync` reproduces the Cache from the
lock alone. A **Consumer** (a project's `.agents/skills/`, or the global config dir) receives skills
as symlinks pointing at their **Source** — a Cache clone for vendored, the Store for authored.

## Workspace layout

```
crates/skilldock-core/   # the library: the model + all ops; no I/O policy beyond the dock
crates/skilldock-cli/    # the `skilldock` (and `sd`) binary — a thin clap wrapper over core ops
crates/skilldock-gui/    # Tauri v2 + React/TS desktop app (built separately; see below)
docs/adr/                # architecture decisions (0001 rewrite, 0002 vendored-as-deps,
                         #   0003 store split + dock layout, 0004 GUI stack)
docs/agents/             # issue tracker, triage labels, domain-doc conventions
CONTEXT.md               # domain glossary (the ubiquitous language)
```

`default-members` is **core + cli only**, so `cargo build` / `cargo test` skip the heavy Tauri build.
Run the GUI through its pnpm Tauri CLI (`cd crates/skilldock-gui && pnpm tauri dev`) — the
`@tauri-apps/cli` devDependency, not the `cargo tauri` subcommand (which needs a separate
`cargo install tauri-cli`).

## Commands

```bash
cargo test                          # core + cli suite (hermetic; uses local git, no network)
cargo fmt --all --check             # formatting gate
cargo clippy --all-targets -- -D warnings   # lint gate
cargo install --path crates/skilldock-cli   # install `skilldock` + `sd` to ~/.cargo/bin
cargo run -p skilldock-gui --example export_bindings   # regenerate the GUI's bindings.ts
(cd crates/skilldock-gui && pnpm tauri dev)            # run the GUI (pnpm Tauri CLI)
```

The installed CLI (`skilldock -h` / `sd -h` on each subcommand):

```
add / remove / update / sync   # vendored lifecycle (declare, drop, re-pin, reproduce Cache)
link / unlink / prune / relink # Consumer links (-g for global, --all across links.txt)
register                       # (de)register a project in links.txt
list / author                  # inventory by provenance; mark/scaffold an authored skill
doctor                         # cross-check dock integrity; errors exit non-zero (the gate)
init                           # fresh-machine bootstrap: clone the data repo, write config, sync
migrate                        # one-shot: convert the old Bash three-manifest repo into the dock
```

## Working on the code

- **Test seam:** build ops test-first against the `skilldock-core` ops seam. Integration tests in
  `crates/skilldock-core/tests/` use the `TempSkilldock` + `GitFixture` helpers in `tests/common/`
  (a real local `git`, no network). Pure logic gets in-module unit tests. Avoid mutating process env
  in tests — construct a `Skilldock` explicitly instead.
- **Central type is `Skilldock`.** Ops take `&Skilldock` explicitly (named `sd`) and never read env;
  only `Skilldock::from_env()` reads `SKILLDOCK_HOME`. Authored originals live at `<store>/skills/<name>`.
- **Shared TOML I/O is in `tomlio`.** The lock rejects glob paths before writing (globs are legal only
  in `skilldock.toml`). JSON (for `migrate` reading the old manifests) uses `serde_json`.
- **`git` is shelled out** via the `git` module. `source::parse_source` turns `owner/repo` / URL into a
  canonical identity + clone URL; `expand` turns declared paths/globs into exact hashed lock skills;
  `cache::ensure_clone` is shared by add/sync. `update`/`migrate` re-clone fresh (a `fetch` does not
  advance a reused clone's HEAD).
- **`doctor`** is read-only by default (`--fix` = sync→relink→prune). `Report::has_errors()` drives the
  non-zero exit. `init` and `migrate` install the data-repo pre-commit gate (`exec skilldock doctor`,
  the const `ops/init.rs::PRE_COMMIT_HOOK`) into the Store.
- **GUI (`skilldock-gui`):** Tauri v2, frontend at the crate root, Rust in `src-tauri/`. Core types the
  GUI surfaces derive `serde::Serialize` + feature-gated `specta::Type` (core `specta` feature, off by
  default; the GUI enables it). `bindings.ts` is committed and guarded by the `bindings_are_current`
  test — regenerate it via the `export_bindings` example. All GUI mutations take a single write-lock.
  The native window can't be verified headlessly; `pnpm tauri build` yields an ad-hoc-signed `.app`.

## Conventions

Commits follow **Conventional Commits** (`feat`/`fix`/`refactor`/`chore(...)`), one logical change
each. The `git-commit` skill (now an authored skill in the Store, linkable via `skilldock link`)
describes the full spec; end AI-assisted commits with a `Co-Authored-By:` trailer.

The **pre-commit gate** is `.githooks/pre-commit` — it runs `cargo fmt --check` + `cargo clippy
-D warnings` + `cargo test`, so commits are blocked while the workspace is red. Enable it in a fresh
clone with `git config core.hooksPath .githooks` (bypass a single commit with `git commit --no-verify`).

## Agent skills

- **Issue tracker:** GitHub issues in `lunaelf/skilldock` (inferred from `git remote`) via `gh`. See
  `docs/agents/issue-tracker.md`.
- **Triage labels:** `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`.
  See `docs/agents/triage-labels.md`.
- **Domain docs:** `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
