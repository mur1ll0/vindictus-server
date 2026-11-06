//! The exports keep the original ordinals (engine.dll imports nmcogame.dll by ordinal).
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    println!("cargo:rustc-cdylib-link-arg=/DEF:{dir}/exports.def");
    println!("cargo:rerun-if-changed=exports.def");
}
