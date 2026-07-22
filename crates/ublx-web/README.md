# ublx-web

Leptos CSR + [leptos-shadcn-ui](https://github.com/cloud-shuttle/leptos-shadcn-ui) catalog UI for `ublx serve` (THI-157).

## Layout

| Module    | Role                                              |
| --------- | ------------------------------------------------- |
| `api`     | Same-origin JSON client + catalog types           |
| `shell`   | Tabs, project path, Last Snapshot footer          |
| `panes`   | 3-pane boxes, list rows, right-pane tabs          |
| `modes/*` | Snapshot / Lenses / Delta / Duplicates / Settings |

## Build (wasm-bindgen — no trunk)

```bash
mise run web          # build dist + serve . --features ui --open
mise run web-check    # fmt + clippy for wasm32
# or:
./crates/ublx-web/build.sh
cargo run -p ublx --features ui -- serve . --open
```

Override asset dir: `UBLX_WEB_DIST=/abs/path/to/dist`.

Default serve without `--features ui` stays API-only.
