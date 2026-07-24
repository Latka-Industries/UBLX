//! Ensure `dist/` exists before rust-embed runs under feature `embed`.

use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let index = manifest.join("dist/index.html");
    println!("cargo:rerun-if-changed=dist");
    println!("cargo:rerun-if-changed=dist/index.html");

    if std::env::var_os("CARGO_FEATURE_EMBED").is_none() {
        return;
    }
    if index.is_file() {
        return;
    }
    panic!(
        "\n\nublx-web: `dist/index.html` missing while building with feature `embed`.\n\
         Run `./crates/ublx-web/build.sh` (or `mise run web`) before \
         `cargo build -p ublx --features ui`.\n"
    );
}
