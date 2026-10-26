fn main() {
    // Replaces Tauri's default manifest, so it must keep the Common Controls v6
    // dependency and DPI awareness that the default one provides.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
