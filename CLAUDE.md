# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

A personal **central store** of Agent Skills. It collects skills from three sources and lets
other projects reference them by **symlink** (not copy), so an update to an original propagates
everywhere and a fix made in any project flows back to the source. The repo itself contains the
skill originals plus Bash tooling to install, inventory, link, and clean up skills.

There is no build step, package manager, or test suite. The "code" is POSIX-ish Bash scripts under
`scripts/`. macOS Bash 3.2 is the target — avoid Bash 4+ features (associative arrays, `${var^^}`, etc.).

## The three skill categories (central concept)

Every skill lives in `.agents/skills/<name>/` (a dir containing `SKILL.md`), but its *provenance*
determines which manifest tracks it. Keeping these straight is the spine of the whole system:

| Source | Manifest (committed) | Dir contents | Update path |
|--------|----------------------|--------------|-------------|
| `npx skills add <pkg>` | `skills-lock.json` (`source` + hash per skill) | real files | `npx skills update` |
| self-authored | `authored.txt` (one name per line) | real files | edit directly |
| GitHub repo (non-npx) | `external.json` (repo + ref + skillPath) | **symlink** into a local clone | `sync-external.sh` |

A directory is treated as a skill **only if it contains `SKILL.md`**. Dirs without one (e.g. a
`*-workspace/` eval/benchmark scratch dir) are ignored by the tooling and gitignored.

`doctor.sh` cross-checks `.agents/skills/` against all three manifests. Anything present but in
none of them is an "orphan" (package leftover, or an unmarked self-authored skill, or an
un-added external). This is why **writing a new skill requires `mark-authored.sh <name>`** and
**importing a GitHub skill requires `add-external.sh`** — otherwise doctor flags them.

## External skills: why symlink + committed manifest

External skills are cloned to a ghq/go-style tree `<root>/<host>/<owner>/<repo>` where root is
`$SKILLS_CODE_ROOT` (default `~/Documents/code`), then symlinked into `.agents/skills/<name>`.
The symlink points at a machine-specific absolute path, so it is **gitignored**; `external.json`
(committed) records the repo URL + subpath so any machine restores the symlink via
`sync-external.sh`. Same split-of-concerns as `skills-lock.json`: manifest in git, materialized
files out of git.

`links.txt` is the analogous machine-local file for the *downstream* side: it records absolute
paths of projects that have ≥1 linked skill (so `prune-all.sh` can find them). `link-skill.sh`
registers a target; `unlink-skill.sh` and `prune-all.sh` de-register a project once it drops to
zero links (`register.sh -r` does it by hand). Also gitignored.

## Commands

All scripts take `-h/--help`. Paths assume repo root as CWD.

```bash
# Inventory / health
scripts/ui/serve.sh [--port <n>]        # local web UI: dashboard + link/unlink/external ops (wraps the scripts)
scripts/test.sh                         # smoke tests (run on a repo copy; mutates nothing real)
scripts/check.sh                        # doctor + gen --check; CI / pre-commit gate
scripts/store/doctor.sh                 # check store vs all 3 manifests; exit!=0 on mismatch
scripts/store/gen-packages.sh           # regenerate PACKAGES.md from the manifests
scripts/store/gen-packages.sh --check   # verify PACKAGES.md is current without writing

# Add / remove skills in the store
npx skills add <owner/repo>             # npm-registry skills -> skills-lock.json
scripts/store/mark-authored.sh <name>   # record a self-written skill in authored.txt
scripts/store/add-external.sh <owner/repo|url> <skill-path-in-repo> [name]   # clone+symlink a GitHub skill
scripts/store/remove-external.sh <name> # undo add-external (symlink + external.json + gitignore)
scripts/store/sync-external.sh [--no-pull]   # restore/update all external skills from external.json

# Link skills INTO a target project (downstream)
scripts/project/link-skill.sh [-f] <target> <skill|package>...   # symlink + auto-register target
scripts/project/link-skill.sh -g <skill|package>...              # install globally (~/.agents + ~/.claude)
scripts/project/unlink-skill.sh [-n] [-g] <target?> <skill|package>...   # remove links (inverse of link)
scripts/project/register.sh [-r] <target>...                     # (de)register a project in links.txt
scripts/project/prune-skills.sh [-n] <target>                    # remove dangling links in one project
scripts/project/prune-skills.sh [-n] -g                          # prune dangling global links (~/.agents + ~/.claude)
scripts/project/prune-all.sh [-n] [-g]                           # prune every project in links.txt (+ global with -g)
```

After any change to the store (npx add/remove, authoring, external add), run `doctor.sh` then
`gen-packages.sh` to keep `PACKAGES.md` current. `PACKAGES.md` is generated — never hand-edit it.

## Working on the scripts

- `scripts/lib/` holds sourced (not executed) helpers. `lock.sh` has the `skills-lock.json` /
  `authored.txt` queries (`lock_*`, `read_authored`) plus `resolve_skill_inputs` (name/package ->
  deduped skill names, shared by link-skill.sh and unlink-skill.sh); `external.sh` has repo-URL parsing
  (`parse_repo`/`clone_dir_for`), `external.json` read/write, and `ensure_gitignore`/`gitignore_remove`.
  Reuse these rather than re-inlining a jq filter — that duplication was the point of the lib.
  JSON is read/written with `jq` when available, falling back to `python3`.
- Each command script resolves `repo_root` by going **two levels up** from its own dir
  (`scripts/<group>/x.sh`). Cross-script calls use `$script_dir/sibling.sh` (e.g. `link-skill.sh`
  invokes `register.sh`, `prune-all.sh` invokes `prune-skills.sh`).
- Scripts use `set -euo pipefail`. Two recurring footguns under this: a `for d in */; do [ test ] &&
  echo; done` loop returns the last iteration's status (neutralize with `|| :` per iteration), and
  empty-array expansion `"${arr[@]}"` errors on Bash 3.2 (guard or branch instead). Both have bitten
  this codebase before.
- Tab-delimited data read back from `external_rows` must be split manually (`${row%%$'\t'*}` …) —
  `IFS=$'\t' read` collapses empty fields because tab is IFS-whitespace.
- Self-referential paths appear in many places: usage headers, runtime fix-it messages, the strings
  `gen-packages.sh` writes into `PACKAGES.md`, and the `ensure_gitignore` marker. If you move or
  rename a script, update all of them (and re-run `gen-packages.sh`, or `--check` will fail).
- `scripts/ui/` is a zero-dependency local web UI (`serve.sh` -> python3-stdlib `server.py` +
  static `index.html`). Contract: reads come from the manifests + directory scans; every mutation
  execs one of the scripts above via a fixed argv mapping in `server.py` (`POST_ROUTES`), so the
  scripts stay the single source of truth. If you add/rename a script or change its flags, update
  the matching endpoint builder and the `== ui ==` checks in `test.sh`. Auth is a per-run token
  from `serve.sh`; the server binds 127.0.0.1 and whitelists every argument.
- `scripts/test.sh` is the smoke suite: it runs the scripts against a throwaway COPY of the repo
  (so `external.json`/`PACKAGES.md`/`links.txt` are never mutated), a fake `$HOME` for global tests,
  and a local `git init` "remote" for external tests — no network. Add a check there when you change
  behavior. It's standalone (not wired into the pre-commit hook, to keep commits fast).

## Conventions

Commits follow Conventional Commits (`feat`/`fix`/`refactor`/`chore(...)`), one logical change each,
as the existing history shows. There is a `git-commit` skill in this very repo
(`.agents/skills/git-commit/`) describing the full spec.

`scripts/install-hooks.sh` points `core.hooksPath` at `scripts/hooks/`, whose `pre-commit` runs
`scripts/check.sh` — so commits are blocked while the repo is inconsistent (orphan dirs, stale
`PACKAGES.md`). After changing skills/manifests, run `gen-packages.sh` or the commit will be
rejected (bypass with `git commit --no-verify`).

## Agent skills

### Issue tracker

Issues are tracked as GitHub issues in `lunaelf/skills` via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary — `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
