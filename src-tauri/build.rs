fn main() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // Embed the same Common Controls v6 dependency for both the app and
        // Rust's unit-test harness (TaskDialogIndirect requires it). Disable
        // Tauri's duplicate RC manifest, keeping its icons/version resources.
        // Cargo's link-arg-tests does not cover an implicit lib-test target.
        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        if let Err(error) = tauri_build::try_build(attributes) {
            panic!("Could not generate Tauri build resources: {error}");
        }
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
    } else {
        tauri_build::build();
    }
}
