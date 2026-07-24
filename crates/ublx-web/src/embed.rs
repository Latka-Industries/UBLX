//! Host-only: pack `dist/` for panza `StaticMount::Embedded`.

use std::collections::HashMap;

use rust_embed::Embed;

/// Built CSR assets (`./build.sh` → `dist/`).
///
/// Compile fails clearly if `dist/index.html` is missing — run `build.sh` first.
#[derive(Embed)]
#[folder = "dist/"]
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
