//! Host-only: pack `assets/web-ui/` for panza `StaticMount::Embedded`.
//!
//! Lives in `ublx` (not `ublx-web`) so crates.io can publish without a path-only
//! workspace dependency. `ublx-web` stays `publish = false` (WASM CSR only).
//! `./crates/ublx-web/build.sh` writes `crates/ublx-web/dist/` then syncs here so
//! the published crate tarball includes the SPA for `cargo install --features ui`.

use std::collections::HashMap;

use rust_embed::Embed;

/// Built CSR assets (`./crates/ublx-web/build.sh` → `assets/web-ui/`).
///
/// Compile fails clearly if `index.html` is missing — run `build.sh` first.
#[derive(Embed)]
#[folder = "assets/web-ui/"]
struct Dist;

/// URL-path keys without a leading slash (`index.html`, `styles/shell.css`, …).
///
/// Skips TypeScript declaration stubs from wasm-bindgen (not served).
pub fn embedded_assets() -> HashMap<String, Vec<u8>> {
    let mut map = HashMap::new();
    for path in Dist::iter() {
        if path.ends_with(".d.ts") {
            continue;
        }
        let Some(file) = Dist::get(path.as_ref()) else {
            continue;
        };
        map.insert(path.to_string(), file.data.into_owned());
    }
    map
}
