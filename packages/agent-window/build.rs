fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("windows")
    ) && matches!(
        std::env::var("CARGO_CFG_TARGET_ENV").as_deref(),
        Ok("msvc")
    ) {
        // Unit tests link Wry without the launcher's tauri-build resources.
        // Common Controls v5 lacks TaskDialogIndirect, so the test executable
        // needs the same v6 dependency as the launcher before it can start.
        // rustc-link-arg also covers lib unit tests; rustc-link-arg-tests only
        // covers integration test targets. These flags stay in this package.
        println!("cargo::rustc-link-arg=/MANIFEST:EMBED");
        println!(concat!(
            "cargo::rustc-link-arg=/MANIFESTDEPENDENCY:",
            "type='win32' name='Microsoft.Windows.Common-Controls' ",
            "version='6.0.0.0' processorArchitecture='*' ",
            "publicKeyToken='6595b64144ccf1df' language='*'"
        ));
    }
}
