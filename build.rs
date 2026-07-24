//! Ensure web `dist/` exists before rust-embed runs under feature `ui`.

use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let index = manifest.join("crates/ublx-web/dist/index.html");
    println!("cargo:rerun-if-changed=crates/ublx-web/dist");
    println!("cargo:rerun-if-changed=crates/ublx-web/dist/index.html");

    if std::env::var_os("CARGO_FEATURE_UI").is_none() {
        return;
    }
    if index.is_file() {
        return;
    }
    panic!(
        "\n\nublx: `crates/ublx-web/dist/index.html` missing while building with feature `ui`.\n\
         Run `./crates/ublx-web/build.sh` (or `mise run web`) before \
         `cargo build --features ui`.\n"
    );
}
