fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/infra/macos/bridge.m");
        cc::Build::new()
            .file("src/infra/macos/bridge.m")
            .flag("-fobjc-arc")
            .flag("-Wall")
            .flag("-Wextra")
            .compile("quicknote_macos_bridge");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=Carbon");
        println!("cargo:rustc-link-lib=framework=WebKit");
    }

    tauri_build::build();
}
