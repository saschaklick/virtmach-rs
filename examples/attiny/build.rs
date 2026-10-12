use std::{ env, path::PathBuf };

fn main() {
    // keep the otherwise unreferenced .mmcu section from src/simavr.rs
    if env::var_os("CARGO_FEATURE_SIMAVR").is_some() {
        println!("cargo:rustc-link-arg=-Wl,--undefined=SIMAVR_MMCU");
    }
    // the program that main.rs puts into the flash, blink.bin unless VIRTMACH_PROGRAM names another,
    // which is how test.sh builds the test programs
    println!("cargo:rerun-if-env-changed=VIRTMACH_PROGRAM");
    let program = env::var_os("VIRTMACH_PROGRAM").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("blink.bin"));
    println!("cargo:rerun-if-changed={}", program.display());
    println!("cargo:rustc-env=VIRTMACH_PROGRAM_PATH={}", program.display());
}
