//! `skilldock-gui` — the Tauri desktop app. A thin adapter over `skilldock-core`
//! (ADR-0001): each `#[tauri::command]` resolves the dock and calls one core
//! operation. `tauri-specta` generates the typed `src/bindings.ts` so the core
//! types carry across the IPC boundary into React (ADR-0004).

use skilldock_core::{self as core, Listing, Skilldock};
use specta_typescript::Typescript;
use tauri_specta::{collect_commands, Builder};

/// Resolve the dock the same way the CLI does: `$SKILLDOCK_HOME` or `~/.skilldock`
/// (ADR-0003 — no first-run picker).
fn skilldock() -> Result<Skilldock, String> {
    Skilldock::from_env().map_err(|e| e.to_string())
}

/// The dashboard read model: skills grouped by provenance.
#[tauri::command]
#[specta::specta]
fn get_state() -> Result<Listing, String> {
    core::list(&skilldock()?).map_err(|e| e.to_string())
}

/// The typed command registry, shared by the app runtime and the bindings
/// export so they can never disagree.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![get_state])
}

/// Export the TypeScript bindings to `path`. Used by the currency test and, in
/// debug builds, on every app start.
pub fn export_bindings(path: &str) -> Result<(), String> {
    specta_builder()
        .export(Typescript::default(), path)
        .map_err(|e| e.to_string())
}

/// Launch the desktop app.
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/bindings.ts")
        .expect("export TypeScript bindings");

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the skilldock GUI");
}

#[cfg(test)]
mod tests {
    /// The committed `bindings.ts` must match what specta generates from the
    /// current command signatures (ADR-0004's CI currency check).
    #[test]
    fn bindings_are_current() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("bindings.ts");
        super::export_bindings(out.to_str().unwrap()).unwrap();
        let generated = std::fs::read_to_string(&out).unwrap();
        let committed =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts"))
                .expect("committed src/bindings.ts must exist");
        assert_eq!(
            generated, committed,
            "src/bindings.ts is stale — regenerate it (cargo tauri dev, or the export path)"
        );
    }
}
