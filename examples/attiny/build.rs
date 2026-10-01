fn main() {
    // keep the otherwise unreferenced .mmcu section from src/simavr.rs
    if std::env::var_os("CARGO_FEATURE_SIMAVR").is_some() {
        println!("cargo:rustc-link-arg=-Wl,--undefined=SIMAVR_MMCU");
    }
}
