fn main() {
    // T0: binary name derives from Cargo.toml (env!("CARGO_BIN_NAME")) — never hardcode it.
    println!("{} v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
}
