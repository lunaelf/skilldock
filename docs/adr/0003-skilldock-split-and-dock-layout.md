# Rename to skilldock; split tool from data; fixed ~/.skilldock layout

The project is renamed **skilldock**. It splits into two repos: a **tool repo** (the Rust
workspace — `skilldock-core` / `-cli` / `-gui`, installed via `cargo install`, binary `skilldock`
with an `sd` alias) and a **data repo** (authored skills + `skilldock.toml` / `skilldock.lock`),
checked out at `~/.skilldock/store`. Vendored clones live at `~/.skilldock/cache/<host>/<owner>/<repo>`,
config at `~/.skilldock/config.toml`, and the whole root relocates with `SKILLDOCK_HOME`. There is
no configuration to *find* anything and no first-run folder picker: the fixed `~/.skilldock` root
is convention. Fresh-machine bootstrap is `skilldock init <data-repo-url>` (clones the data repo
into the dock) then `skilldock sync` (populates the Cache from the lock and creates links).

**Why:** it is the cargo model exactly — the tool is an installed binary, `~/.skilldock` is pure
data + cache. A fixed root removes the GUI folder-picker friction, and keeping the tool's Rust dev
repo out of a hidden dotdir keeps day-to-day development normal.

## Considered options

- **Option 1 — single repo in the dev tree + a config file recording its path.** Rejected: also
  picker-free, but keeps tool source and data fused in one repo. We preferred the clean cargo-style
  split.
- **2a — one repo living inside the dock (`~/.skilldock/store` holds Rust source + `target/` +
  data).** Rejected as dominated: it shoves the dev repo and build artifacts into the dock,
  defeating the "clean dock" appeal that motivated Option 2.

## Consequences

- Two repos to manage instead of one. The current repo is split: tooling → tool repo, authored
  skills + manifest → data repo.
- A one-shot `tar czf dock.tgz ~/.skilldock` now snapshots authored skills, the manifest, **and**
  every vendored clone, restorable offline — this materially softens the upstream-disappearance
  ("left-pad") risk called out in ADR-0002.
- The GUI and CLI both anchor on `~/.skilldock` with zero locator config.
