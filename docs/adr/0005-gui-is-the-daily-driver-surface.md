# GUI is the daily-driver surface; init & migrate stay CLI-only

The `skilldock-gui` desktop app is made **capability-complete for daily operations**: everything you
touch repeatedly — `add` (with `--ref`), `remove`, `update` (all *and* per-repo), `sync`, `link`
(with `--force`), `unlink`, `prune`, `relink`, `register` / `deregister`, `author`, and `doctor`
(with `--verify` / `--fix`) — is reachable from the GUI. The one-shot bootstrap commands `init`
(fresh-machine clone + config) and `migrate` (Bash-era one-time conversion) are **deliberately left
CLI-only**. In-app editing of `SKILL.md` is also out of scope; authoring scaffolds the file and
reveals it in the OS file manager for you to edit in your own editor.

To make project linking practical without retyping paths, the GUI surfaces the **Registry**
(`links.txt`) as a first-class, selectable list of Consumers (registered projects + `Global`).
Selecting a Consumer scopes `link` / `unlink` / `prune` / `relink` to it and shows each skill's link
state (linked / not, healthy / dangling) so the buttons become meaningful toggles instead of
fire-and-forget. This replaces the old single free-text path box.

**Why:** the goal is to run daily skill management entirely from the GUI. `init` runs on a fresh
machine *before* the GUI would be opened (chicken-and-egg: no dock to point a window at yet), and
`migrate` is a historical one-shot — building either into the GUI is cost with almost no recurring
payoff. Recording the boundary stops a future reader who sees a dozen commands in the GUI but not
these two from "completing" it.

## Considered options

- **Full parity, including `init` + `migrate` in the GUI.** Rejected: `init` has the chicken-and-egg
  problem (bootstrapping a dock from a window that needs a dock), and `migrate` is a one-time path
  that will bit-rot in the UI. Both belong to the terminal a user is already in on a fresh machine.
- **Keep the free-text path box, just add the missing buttons.** Rejected: minimal, but you keep
  retyping project paths and the Registry stays invisible — you can't see what's linked where without
  falling back to `doctor` or the filesystem, which defeats "GUI-only daily use."
- **A skill×Consumer matrix of checkboxes.** Rejected for now: most powerful at-a-glance view, but the
  heaviest to build; the selectable-Consumer + per-skill-state model delivers the same daily workflow
  for far less.

## Consequences

- Core grows two read seams the CLI never needed as commands: a public accessor to enumerate the
  Registry (previously crate-private `registry::read`), and a per-Consumer "which skills are linked,
  and are they healthy" read used to render link state.
- New Tauri commands `register` / `deregister` plus the two reads; `relink` / `prune` / `author`
  already exist in the backend and only needed UI. `add` / `update` / `doctor` / `link` command
  signatures gain the previously hard-coded parameters.
- Two Tauri plugins enter the GUI crate — `tauri-plugin-dialog` (native folder picker for
  registering a project) and `tauri-plugin-opener` (Reveal in Finder for authored skills) — each with
  a capability permission. This stays within ADR-0004's "build-time/desktop toolchain confined to the
  GUI crate"; `bindings.ts` is regenerated.
- The GUI now owns a write surface for the Registry, so its mutations run under the same single
  write-lock as every other GUI mutation (ADR-0004's "one write at a time").
