use std::path::Path;
use std::process::Command;

fn main() {
    slint_build::compile("ui/app.slint").unwrap();

    println!("cargo:rerun-if-changed=custom-libs/wine_hook.c");
    println!("cargo:rerun-if-changed=custom-libs/Makefile");

    let custom_libs = Path::new("custom-libs");
    if custom_libs.join("wine_hook.c").exists() && custom_libs.join("Makefile").exists() {
        let _ = Command::new("make")
            .args(["-C", "custom-libs", "all"])
            .status();
    }
}
