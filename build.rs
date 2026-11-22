// Embeds icon, manifest and version info using windres when available.
// Missing windres is not an error: the exe just lacks the resources.
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/breakguard.rc");
    println!("cargo:rerun-if-changed=assets/breakguard.manifest");
    println!("cargo:rerun-if-changed=assets/breakguard.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let obj = out.join("breakguard_res.o");
    let ok = Command::new("windres")
        .current_dir("assets")
        .args(["--input", "breakguard.rc", "--output-format=coff", "--output"])
        .arg(&obj)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", obj.display());
    } else {
        println!("cargo:warning=windres unavailable: exe built without icon/manifest");
    }
}
