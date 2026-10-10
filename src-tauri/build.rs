fn main() {
    #[cfg(windows)]
    let attrs = tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new().app_manifest(
            include_str!("windows-app-manifest.xml").to_string(),
        ),
    );

    #[cfg(not(windows))]
    let attrs = tauri_build::Attributes::new();

    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}