# GUI frontend is React + TypeScript, typed across IPC with tauri-specta

The `skilldock-gui` frontend is built with React + TypeScript + Vite, and `tauri-specta` generates
TypeScript types from the Rust command signatures so the `skilldock-core` types carry across the
IPC boundary into React. We do **not** reuse the old vanilla `index.html`.

**Why:** the whole point of the Rust rewrite (ADR-0001) is a typed, testable codebase; a vanilla
`index.html` would have been the one untyped, untested island floating on top of typed Rust.
React + TS + specta completes that goal instead of puncturing it — a single type chain runs
core → typed IPC → typed React, so a change to a Rust command signature surfaces as a TS compile
error.

## Considered options

- **Reuse the vanilla `index.html` via `withGlobalTauri` (fetch → invoke).** Rejected: fastest to a
  working GUI and zero JS toolchain, but leaves an untyped/untested island and a stringly-typed
  `invoke('cmd', {...})` boundary.
- **React without specta.** Rejected: it only moves the untyped seam from hand-rolled DOM to the
  `invoke` boundary — the compiler still can't check that the two sides of the IPC agree.

## Consequences

- The added pnpm/Vite toolchain is **build-time only** — it is not the runtime Node that ADR-0002
  retired, and the shipped app bundles static assets with zero Node at runtime.
- The JS toolchain is isolated to the `skilldock-gui` crate; `skilldock-cli` and `skilldock-core`
  stay pure Rust with no JS.
- The existing ~886-line vanilla UI is discarded and rewritten in React.
- A pnpm/npm-registry maintenance surface is introduced, confined to the GUI crate.
