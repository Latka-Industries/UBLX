//! Ensure web UI assets exist before rust-embed runs under feature `ui`.

use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let index = manifest.join("assets/web-ui/index.html");
    println!("cargo:rerun-if-changed=assets/web-ui");
    println!("cargo:rerun-if-changed=assets/web-ui/index.html");

    if std::env::var_os("CARGO_FEATURE_UI").is_none() {
        return;
    }
    if index.is_file() {
        return;
    }
    panic!(
        "\n\nublx: `assets/web-ui/index.html` missing while building with feature `ui`.\n\
         Run `./crates/ublx-web/build.sh` (or `mise run web`) before \
         `cargo build --features ui` / `cargo install --features ui`.\n"
    );
}
