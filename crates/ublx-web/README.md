# ublx-web

Leptos CSR + [leptos-shadcn-ui](https://github.com/cloud-shuttle/leptos-shadcn-ui) catalog UI for `ublx serve` (THI-157).

`publish = false` — WASM app only. The host embeds via `ublx` feature `ui` / [`web_embed.rs`](../../src/cli/serve/web_embed.rs) reading **`assets/web-ui/`** (synced from `dist/` by `build.sh`). Those assets ship in the crates.io tarball for `cargo install ublx --features ui`.

## Layout

| Module        | Role                                                           |
| ------------- | -------------------------------------------------------------- |
| `api`         | Same-origin JSON client + catalog types                        |
| `nav`         | `MainMode` tabs + `/?mode=` (never UI-navigate to `/delta`, …) |
| `shell`       | Tabs, project path, Last Snapshot footer                       |
| `panes`       | 3-pane boxes, list rows, right-pane tabs                       |
| `viewer_find` | Shift+S in-pane find strip + DOM highlights                    |
| `modes/*`     | Snapshot / Lenses / Delta / Duplicates / Settings              |

## Build (wasm-bindgen — no trunk)

Needs **Rust** (wasm32) + **wasm-bindgen-cli** + **Node/npm** (Tailwind build — no CDN).

```bash
mise run web          # build dist + serve . with UBLX_WEB_DIST (Dir, no re-embed)
mise run web-check    # fmt + clippy for wasm32
# or:
./crates/ublx-web/build.sh
# Ship / Embedded (one binary — rebuild host after dist changes):
cargo run -p ublx --features ui -- serve . --open
# Dev Dir override:
UBLX_WEB_DIST=$PWD/crates/ublx-web/dist cargo run -p ublx --features ui -- serve . --open
```

`build.sh` writes `dist/tailwind.css` via `@tailwindcss/cli` **4.3.3** (`styles/tailwind-input.css` + `tailwind.config.js`; scans `src/**/*.rs` + leptos-shadcn sources). Default serve without `--features ui` stays API-only.
