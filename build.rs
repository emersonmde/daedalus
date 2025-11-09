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
            .args([
                "-target",
                "aarch64-none-elf",
                "-c",
                "src/arch/aarch64/boot.s",
                "-o",
            ])
            .arg(out_dir.join("boot.o"))
            .status()
            .expect("Failed to assemble boot.s");

        if !status.success() {
            panic!("Assembly of boot.s failed");
        }

        // Assemble exceptions.s
        let status = Command::new("clang")
            .args([
                "-target",
                "aarch64-none-elf",
                "-c",
                "src/arch/aarch64/exceptions.s",
                "-o",
            ])
            .arg(out_dir.join("exceptions.o"))
            .status()
            .expect("Failed to assemble exceptions.s");

        if !status.success() {
            panic!("Assembly of exceptions.s failed");
        }

        // Create libarch.a from boot.o and exceptions.o
        let status = Command::new("ar")
            .args(["crs"])
            .arg(out_dir.join("libarch.a"))
            .arg(out_dir.join("boot.o"))
            .arg(out_dir.join("exceptions.o"))
            .status()
            .expect("Failed to create libarch.a");

        if !status.success() {
            panic!("Failed to create archive");
        }

        // Link the object files
        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-lib=static:+whole-archive=arch");
    }

    // Re-run if assembly or linker script changes
    println!("cargo:rerun-if-changed=src/arch/aarch64/boot.s");
    println!("cargo:rerun-if-changed=src/arch/aarch64/exceptions.s");
    println!("cargo:rerun-if-changed=linker.ld");
}
