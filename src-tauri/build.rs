use std::path::Path;

fn main() {
    sidecar_placeholder();
    // Replaces Tauri's default manifest, so it must keep the Common Controls v6
    // dependency and DPI awareness that the default one provides.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}

/// Tauri checks that every sidecar exists at compile time. `pnpm sidecar`
/// builds the real apex-timer; for plain `cargo test`/`clippy` in debug, an
/// empty placeholder keeps the build working. Release builds never get one,
/// so a missing helper fails the build instead of shipping an empty file.
fn sidecar_placeholder() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let ext = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let path = format!("binaries/apex-timer-{target}{ext}");
    println!("cargo:rerun-if-changed={path}");
    if std::env::var("PROFILE").as_deref() == Ok("debug") && !Path::new(&path).exists() {
        std::fs::create_dir_all("binaries").expect("binaries folder");
        std::fs::write(&path, b"").expect("sidecar placeholder");
    }
}
