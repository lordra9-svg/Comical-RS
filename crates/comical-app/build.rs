//! Embeds the Windows icon, version information and application manifest.
//!
//! Only does anything when the target is Windows. Without this the executable
//! has no `.rsrc` section at all: Explorer falls back to a generic icon, the
//! Properties dialog shows no product or version, and Windows applies default
//! DPI and compatibility behavior instead of the declared one.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/comical.ico");
    println!("cargo:rerun-if-changed=assets/windows.manifest");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_owned());
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("assets/comical.ico")
        .set_manifest_file("assets/windows.manifest")
        .set("FileDescription", "Comical comic book reader")
        .set("ProductName", "Comical")
        .set("OriginalFilename", "comical.exe")
        .set("InternalName", "comical")
        .set("FileVersion", &version)
        .set("ProductVersion", &version)
        .set(
            "LegalCopyright",
            "MIT licensed. Copyright (c) Comical-RS Contributors",
        );

    resource
        .compile()
        .expect("failed to embed Windows resources; is windres (GNU) or rc.exe (MSVC) installed?");
}
