//! Regenerate the committed TypeScript bindings without launching a window.
//! Run: `cargo run -p skilldock-gui --example export_bindings`.

fn main() {
    // Absolute (manifest-relative) so it works regardless of the invocation CWD.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");
    skilldock_gui_lib::export_bindings(path).expect("export TypeScript bindings");
    println!("wrote {path}");
}
