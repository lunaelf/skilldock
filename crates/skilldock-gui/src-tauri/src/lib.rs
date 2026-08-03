//! `skilldock-gui` — the Tauri desktop app. A thin adapter over `skilldock-core`
//! (ADR-0001): each `#[tauri::command]` resolves the dock and calls one core
//! operation. `tauri-specta` generates the typed `src/bindings.ts` so the core
//! types carry across the IPC boundary into React (ADR-0004).
//!
//! Every mutation takes the single write lock first, so only one write touches
//! the dock at a time ("one write at a time").

use std::path::Path;
use std::sync::Mutex;

use serde::Deserialize;
use skilldock_core::{
    self as core, AddOutcome, AddRequest, AuthorOutcome, Consumer, DoctorOptions, LinkOutcome,
    Listing, PruneOutcome, RelinkOutcome, RemoveOutcome, Report, SkillLinkStatus, SkillSpec,
    Skilldock, Source, SyncOutcome, UnlinkOutcome, UpdateOutcome,
};
use specta_typescript::Typescript;
use tauri::State;
use tauri_specta::{collect_commands, Builder};

/// Shared app state: the single-mutation lock (ADR-0004's "one write at a time").
#[derive(Default)]
struct AppState {
    write_lock: Mutex<()>,
}

/// Resolve the dock the same way the CLI does: `$SKILLDOCK_HOME` or `~/.skilldock`
/// (ADR-0003 — no first-run picker).
fn skilldock() -> Result<Skilldock, String> {
    Skilldock::from_env().map_err(|e| e.to_string())
}

/// Take the write lock; held for the duration of one mutation so only one write
/// touches the dock at a time. A second concurrent write blocks here until the
/// first releases; the error path is only reachable if a prior write panicked
/// (a poisoned lock).
fn write_guard(state: &AppState) -> Result<std::sync::MutexGuard<'_, ()>, String> {
    state
        .write_lock
        .lock()
        .map_err(|_| "a previous write panicked; restart the app".to_string())
}

/// How the frontend names a Consumer: a project path, or the global config.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ConsumerArg {
    Project { path: String },
    Global,
}

impl ConsumerArg {
    fn resolve(self) -> Result<Consumer, String> {
        match self {
            ConsumerArg::Project { path } => Ok(Consumer::project(path)),
            ConsumerArg::Global => Consumer::global_from_home().map_err(|e| e.to_string()),
        }
    }
}

// ---- read ------------------------------------------------------------------

/// The dashboard read model: skills grouped by provenance.
#[tauri::command]
#[specta::specta]
fn get_state() -> Result<Listing, String> {
    core::list(&skilldock()?).map_err(|e| e.to_string())
}

/// The registered project Consumers (the Registry / `links.txt`) as path
/// strings. `Global` is never registered; the frontend adds it as an
/// always-available entry.
#[tauri::command]
#[specta::specta]
fn registered_consumers() -> Result<Vec<String>, String> {
    Ok(core::registered_consumers(&skilldock()?)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// Each dock skill's link state (unlinked / linked / dangling) in `consumer`,
/// for the per-skill Link/Unlink toggle.
#[tauri::command]
#[specta::specta]
fn link_status(consumer: ConsumerArg) -> Result<Vec<SkillLinkStatus>, String> {
    core::link_status(&skilldock()?, &consumer.resolve()?).map_err(|e| e.to_string())
}

/// The Store directory of an authored skill — the reveal target for "Reveal in
/// Finder". Resolves the dock layout in Rust (ADR-0003), so the frontend never
/// hard-codes dock paths; it just hands the result to the opener plugin.
#[tauri::command]
#[specta::specta]
fn authored_skill_dir(name: String) -> Result<String, String> {
    Ok(skilldock()?
        .authored_skill_dir(&name)
        .to_string_lossy()
        .into_owned())
}

// ---- mutations (each takes the write lock) ---------------------------------

#[tauri::command]
#[specta::specta]
fn add(
    state: State<'_, AppState>,
    repo: String,
    skills: Vec<String>,
    git_ref: Option<String>,
) -> Result<AddOutcome, String> {
    let _g = write_guard(&state)?;
    let source: Source = core::parse_source(&repo).map_err(|e| e.to_string())?;
    core::add(
        &skilldock()?,
        AddRequest {
            source,
            git_ref,
            skills: skills.into_iter().map(SkillSpec::Path).collect(),
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn remove(state: State<'_, AppState>, target: String) -> Result<RemoveOutcome, String> {
    let _g = write_guard(&state)?;
    core::remove(&skilldock()?, &target).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn update(state: State<'_, AppState>, repos: Vec<String>) -> Result<UpdateOutcome, String> {
    let _g = write_guard(&state)?;
    core::update(&skilldock()?, &repos).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn sync(state: State<'_, AppState>) -> Result<SyncOutcome, String> {
    let _g = write_guard(&state)?;
    core::sync(&skilldock()?).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn link(
    state: State<'_, AppState>,
    consumer: ConsumerArg,
    skills: Vec<String>,
    force: bool,
) -> Result<LinkOutcome, String> {
    let _g = write_guard(&state)?;
    core::link(&skilldock()?, &consumer.resolve()?, &skills, force).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn unlink(
    state: State<'_, AppState>,
    consumer: ConsumerArg,
    skills: Vec<String>,
) -> Result<UnlinkOutcome, String> {
    let _g = write_guard(&state)?;
    core::unlink(&skilldock()?, &consumer.resolve()?, &skills).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn relink(state: State<'_, AppState>, consumer: ConsumerArg) -> Result<RelinkOutcome, String> {
    let _g = write_guard(&state)?;
    core::relink(&skilldock()?, &consumer.resolve()?).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn prune(state: State<'_, AppState>, consumer: ConsumerArg) -> Result<PruneOutcome, String> {
    let _g = write_guard(&state)?;
    core::prune(&skilldock()?, &consumer.resolve()?).map_err(|e| e.to_string())
}

/// Register a project as a Consumer (adds it to `links.txt`). Errors if the path
/// does not exist. Returns whether it was newly added.
#[tauri::command]
#[specta::specta]
fn register(state: State<'_, AppState>, consumer: String) -> Result<bool, String> {
    let _g = write_guard(&state)?;
    core::register(&skilldock()?, Path::new(&consumer)).map_err(|e| e.to_string())
}

/// Deregister a project Consumer (removes it from `links.txt`); works whether or
/// not the path still exists. Returns whether it was present.
#[tauri::command]
#[specta::specta]
fn deregister(state: State<'_, AppState>, consumer: String) -> Result<bool, String> {
    let _g = write_guard(&state)?;
    core::deregister(&skilldock()?, Path::new(&consumer)).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn author(state: State<'_, AppState>, name: String) -> Result<AuthorOutcome, String> {
    let _g = write_guard(&state)?;
    core::author(&skilldock()?, &name).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
fn doctor(
    state: State<'_, AppState>,
    verify: bool,
    fix: bool,
    consumers: bool,
) -> Result<Report, String> {
    // doctor with `fix` writes; guard it too.
    let _g = write_guard(&state)?;
    core::doctor(
        &skilldock()?,
        DoctorOptions {
            verify,
            fix,
            consumers,
        },
    )
    .map_err(|e| e.to_string())
}

/// The typed command registry, shared by the app runtime and the bindings
/// export so they can never disagree.
fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        get_state,
        registered_consumers,
        link_status,
        authored_skill_dir,
        add,
        remove,
        update,
        sync,
        link,
        unlink,
        relink,
        prune,
        register,
        deregister,
        author,
        doctor
    ])
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
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
            "src/bindings.ts is stale — regenerate it (cargo tauri dev, or the export example)"
        );
    }
}
