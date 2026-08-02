# skilldock

A personal tool for managing [Agent Skills](https://www.skills.sh/). It keeps the originals of
skills you wrote, caches the ones you pull from other people's repos, and **links** both into the
projects that use them — by symlink, not copy — so:

- **one update, everywhere** — re-pin a source or edit an original and every project that links it
  follows automatically;
- **fixes flow back** — because links point at the real original, a change made while working in a
  project is a change to the source, ready to commit upstream;
- **projects stay lean** — each project links only the skills it actually uses.

`skilldock` is a single `cargo install`ed binary. The data it manages lives in a per-user **dock** at
`~/.skilldock`, separate from this repo. See [`CONTEXT.md`](CONTEXT.md) for the full vocabulary and
[`docs/adr/`](docs/adr/) for the design.

## Install

```bash
cargo install --path crates/skilldock-cli   # installs `skilldock` and the `sd` alias
# ensure ~/.cargo/bin is on your PATH
```

## Concepts

A **skill** is a directory containing `SKILL.md`. Each has a **provenance**:

- **authored** — the original lives in your **Store** (the data repo, checked out at
  `~/.skilldock/store`) and is edited there directly;
- **vendored** — the original lives in someone else's GitHub repo, cloned into the **Cache**
  (`~/.skilldock/cache/<host>/<owner>/<repo>`) pinned to a ref.

`skilldock.toml` (in the Store) declares what the dock should contain; `skilldock.lock` records the
exact commit + content hash per vendored skill, so the Cache can be reproduced from the lock alone.

## Getting started

On a fresh machine, bootstrap the dock from your data repo:

```bash
skilldock init https://github.com/lunaelf/skills.git   # clone the Store, write config, sync the Cache
```

Then link skills into a project (a **Consumer**):

```bash
skilldock link ~/code/my-project tdd code-review     # symlink specific skills
skilldock link -g hv-analysis                        # or install globally (~/.agents + ~/.claude)
```

## Command overview

Run `skilldock <cmd> -h` for details.

| Area | Commands |
|------|----------|
| Vendored sources | `add` · `remove` · `update` (re-pin) · `sync` (reproduce Cache from the lock) |
| Consumer links | `link` · `unlink` · `prune` (drop dangling) · `relink` (re-point) · `register` |
| Authoring | `author` (mark/scaffold an authored skill) · `list` |
| Health | `doctor` (integrity check; `--fix` = sync→relink→prune) |
| Bootstrap | `init` (fresh machine) · `migrate` (one-shot import of the old Bash repo) |

`-g` targets the global config; `--all` fans a `relink`/`prune` across every project in the dock's
`links.txt`.

## Dock layout

```
~/.skilldock/
  store/                     # the data repo (github.com/lunaelf/skills)
    skilldock.toml           #   what the dock should contain
    skilldock.lock           #   resolved commits + content hashes
    skills/<name>/           #   authored skill originals
  cache/<host>/<owner>/<repo>/   # cloned sources for vendored skills, pinned to a ref
  config.toml                # the data-repo remote + preferences
  links.txt                  # projects that link skills (machine-local, absolute paths)
```

## Development

```bash
cargo test                                 # core + cli suite (hermetic — local git, no network)
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
git config core.hooksPath .githooks        # enable the pre-commit gate (runs the three above)
```

The desktop GUI lives in `crates/skilldock-gui` (Tauri v2 + React/TS) and is driven through its
npm Tauri CLI (already a devDependency):

```bash
cd crates/skilldock-gui
npm install            # first time only
npm run tauri dev      # run the app (hot-reloads); npm run tauri build to bundle
```

(Prefer the `cargo tauri` form? `cargo install tauri-cli --version "^2"`, then `cargo tauri dev`.)
See `CLAUDE.md` for the workspace conventions and `docs/adr/` for the architecture.
