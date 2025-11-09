use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();

    // Only compile assembly for aarch64 target
    if target.starts_with("aarch64") {
        // Assemble boot.s
        let status = Command::new("clang")
            .args(&[
                "-target",
                "aarch64-none-elf",
                "-c",
                "src/boot.s",
                "-o",
            ])
            .arg(&out_dir.join("boot.o"))
            .status()
            .expect("Failed to assemble boot.s");

        if !status.success() {
            panic!("Assembly failed");
        }

        // Create libboot.a from boot.o
        let status = Command::new("ar")
            .args(&["crs"])
            .arg(out_dir.join("libboot.a"))
            .arg(out_dir.join("boot.o"))
            .status()
            .expect("Failed to create libboot.a");

        if !status.success() {
            panic!("Failed to create archive");
        }

        // Link the boot object file
        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-lib=static:+whole-archive=boot");
    }

    // Re-run if boot.s or linker.ld changes
    println!("cargo:rerun-if-changed=src/boot.s");
    println!("cargo:rerun-if-changed=linker.ld");
}
