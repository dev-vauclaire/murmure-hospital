fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../resources/hospital/user-guide");

    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS")? == "windows"
        && std::env::var("CARGO_CFG_TARGET_ENV")? == "msvc";
    let attributes = if windows_msvc {
        // Embed the manifest through the linker so library unit tests receive it too.
        // Disable only Tauri's manifest to avoid embedding it twice in the application.
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    } else {
        tauri_build::Attributes::new()
    };

    tauri_build::try_build(attributes)?;

    if windows_msvc {
        let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?)
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    Ok(())
}
