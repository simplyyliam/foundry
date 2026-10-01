fn main() {
    let mut attributes = tauri_build::Attributes::new();

    // On Windows MSVC the app manifest (Common Controls v6) is embedded via linker args instead
    // of tauri-build's resource file, so that test executables get it too. Without it, tests
    // that link Tauri exit with STATUS_ENTRYPOINT_NOT_FOUND.
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows_msvc {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        println!("cargo:rustc-link-arg=/WX");
    }

    if let Err(err) = tauri_build::try_build(attributes) {
        eprintln!("tauri-build failed: {err:#}");
        std::process::exit(1);
    }
}
