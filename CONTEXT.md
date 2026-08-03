# Skilldock

A personal system for managing Agent Skills. It keeps the originals of skills you wrote, caches
the ones you pull from other people's repos, and links both into the projects that use them so an
update to an original flows through without copying. `skilldock` is also the name of the tool
(a `cargo install`ed binary) that operates the system.

## Language

**Skill**:
A directory containing a `SKILL.md` file — the atomic unit the system manages. A directory
without `SKILL.md` is not a skill.

**Provenance**:
Where a skill's original comes from — `vendored` or `authored`. It is the only essential
distinction between skills; it decides which Source owns each one and how it is updated.
_Avoid_: source type, category, kind

**Vendored skill**:
A skill whose original lives in someone else's GitHub repo, cached pinned to a ref. Unifies what
the pre-Rust tooling split into two separate categories.
_Avoid_: npx skill, external skill, package, npm skill

**Authored skill**:
A skill whose original lives in the Store and is edited there directly.
_Avoid_: self-authored, local skill, own skill

**Skilldock**:
The whole per-user system, rooted at `~/.skilldock` (override with `SKILLDOCK_HOME`). Holds the
Store, the Cache, and config. The `skilldock` tool is named after it and is installed separately —
it does not live inside the dock.
_Avoid_: dock, home, root

**Store**:
The data repo checked out at `~/.skilldock/store`: authored skill originals plus the manifest
(`skilldock.toml` / `skilldock.lock`). It holds no tooling and no vendored files.
_Avoid_: registry, library, catalog, hub

**Cache**:
`~/.skilldock/cache/<host>/<owner>/<repo>`: the cloned source repos for vendored skills, pinned to
a ref. One clone serves every skill that repo provides.
_Avoid_: skilldock (the whole system), clone tree

**Source**:
The git repo a skill's original lives in — a Cache clone for a vendored skill, the Store for an
authored one.
_Avoid_: origin, upstream

**Consumer**:
A project's `.agents/skills/` (or the global config dir) that receives skills as links pointing at
their Source.
_Avoid_: target, downstream, project

**Registry**:
The machine-local set (`links.txt`) of project Consumers that hold at least one link — what
`register` / `deregister` maintain and what `relink --all` / `prune --all` iterate. Global Consumers
are never in it.
_Avoid_: links file, index, catalog

**Link**:
A symlink from a Consumer to a skill in its Source — a Cache clone for vendored, the Store for
authored — so updates flow to the Consumer without copying.
_Avoid_: install, copy
