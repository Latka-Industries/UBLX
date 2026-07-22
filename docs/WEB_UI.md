# Embedded web UI for `ublx serve` (THI-157 / v0.2.0)

Design note for the optional catalog browser. Stack and packaging are locked below. **First-pass shell** lives on `dev` (`crates/ublx-web/`: chrome + Snapshot categories/entries). Port the remaining modes against the TUI map in [`TUI_STRUCTURE.md`](TUI_STRUCTURE.md).

**Linear:** [THI-157](https://linear.app/thicclatka/issue/THI-157/web-ui-leptos-feature-flagged-for-ublx-serve-v020)  
**Depends on:** [THI-156](https://linear.app/thicclatka/issue/THI-156/ublx-serve-local-http-api-over-ublx) (Done — JSON API + panza)  
**Target release:** **v0.2.0**

---

## Goal

Optional **embedded catalog browser** for `ublx serve`: browse the `.ublx` catalog in a browser with UBLX theme personality — not a generic SaaS dashboard. All-Rust stack so a **feature-flagged build can ship one binary** (API + UI).

Default crates.io / Homebrew installs stay **API-only** (`StaticMount::None`). The UI is opt-in via Cargo feature **`ui`**.

---

## Locked decisions

| Decision | Choice | Why |
| -------- | ------ | --- |
| Feature name | **`ui`** | Short; documents as `cargo build --features ui` |
| Framework | **Leptos** (CSR) | All-Rust; embeds cleanly; no Node SPA host |
| Component library | **[leptos-shadcn-ui](https://github.com/cloud-shuttle/leptos-shadcn-ui)** | Real widgets (table, tabs, input, …) for Leptos 0.8+; no hand-rolled CSS kit |
| HTTP shell | **panza** only | No second bind/health/static stack |
| Static mount | `StaticMount::Embedded` when `ui`; `None` otherwise | panza already SPA-falls-back to `index.html` |
| Data path | Same-origin **JSON API** from THI-156 | UI is a client of `/entries`, `/delta`, … — not Leptos server functions |
| Not chosen | Svelte / Vite / Next as the app host; egui-as-web; Thaw; Leptonic | Host stays Leptos CSR + panza; shadcn-ui is the widget layer |

### CSR vs SSR

Prefer **CSR WASM + embed**, not full SSR / `cargo-leptos` dual-compile of the `ublx` binary.

- Serve already owns the Axum router via panza; SSR would fight that shell.
- Localhost catalog browser does not need SEO or first-paint HTML from Rust.
- UI talks to existing routes with `fetch` — matches “same-origin JSON API.”

Dev loop may use `StaticMount::Dir("…/dist")` so assets rebuild without re-embedding every tweak.

---

## Cargo / packaging

```toml
[features]
default = ["zahir-netcdf"]   # unchanged — no UI
ui = []                      # embeds Leptos SPA; implies using serve’s static mount
```

Rules:

- Default binary includes `ublx serve` **API**; **no** Leptos / WASM deps.
- `--features ui` enables embedded assets and switches serve to `StaticMount::Embedded`.
- Do **not** hide API-only serve behind `ui`.
- Suggested layout: workspace crate **`crates/ublx-web/`** (wasm32 CSR app). Host `ublx` depends on it only under `ui` for asset embedding.

### Build story (to document when implemented)

1. Build CSR assets (Trunk or equivalent) → `dist/` (`index.html` + WASM/JS).
2. `cargo build --features ui` embeds `dist/` into the binary.
3. Optional mise / script wraps both steps.

Default `cargo build` (no `ui`) stays a single native compile.

---

## Components & layout

**UI kit:** [leptos-shadcn-ui](https://github.com/cloud-shuttle/leptos-shadcn-ui) (Leptos 0.8+). Prefer published component crates over inventing primitives.

| Need | Likely pieces |
| ---- | ------------- |
| Mode chrome | Tabs via [`nav`](../crates/ublx-web/src/nav.rs) (`MainMode` signal + optional `/?mode=`) |
| Entries | Table or dense list + Input / Select for filters |
| Detail | Card or aside panel; preformatted Zahir JSON |
| Routing | Stay on `/`; never use API path segments (`/delta`, `/lenses`, …) as UI pages — see `RESERVED_API_PATH_SEGMENTS` |

**TUI → web:** Mirror TUI chrome (tabs, path gap, 3-pane boxes, Last Snapshot). Full layout roles are in [`TUI_STRUCTURE.md`](TUI_STRUCTURE.md) — prefer that over inventing a dashboard layout.

| TUI | Web |
| --- | --- |
| Main tabs + brand | In-app mode tabs + `UBLX` |
| Indexed root gap | Project path under tabs |
| Categories / Contents / Right | 3-pane shell (`ThreePane`) |
| Delta / Lenses / Duplicates | Same 3-pane roles per mode |
| Settings | Scope + **button/control rows** + live read-only TOML (no text editor) |
| Theme / brand | CSS tokens from `Palette` |
| File viewers (md/pdf/image) | Later; start with text / Zahir / meta |

---

## Theming

Map TUI [`Palette`](../src/themes/mod.rs) into **leptos-shadcn-ui / CSS theme tokens** (same roles as below). Theme switcher swaps token sets — do not hardcode a generic purple SaaS skin as the product look.

| Token role (proposed) | `Palette` field |
| --------------------- | --------------- |
| background / page | `background` |
| foreground / text | `text` |
| focus / ring / accent | `focused_border` |
| tab active fg/bg | `tab_active_fg` / `tab_active_bg` |
| tab inactive | `tab_inactive_bg` |
| muted / hint | `hint` |
| search accent | `search_text` |
| popover / panel | `popup_bg` |
| delta added / mod / removed | `delta_*` |
| brand | `title_brand` |

Reuse hex helpers under `themes::color_utils` (`color_to_hex6` / `rgb_to_hex6`) when generating tokens.

**MVP themes:** at least default dark (**Oblivion Ink**) + one light. Full palette parity can follow.

If shadcn-ui’s default toolchain expects Tailwind utilities for layout, that is OK **as a styling layer under Leptos** — the app host is still Leptos CSR + panza, not a Vite/Svelte frontend.

---

## MVP UI surface

Catalog browser that **looks like the TUI**. Dense, path-first. Skip rich file viewers (md/pdf/image) for Done; text / Zahir / meta is enough.

**Landed (first pass on `dev`):**

- [x] App shell: main tabs, brand, project path, Last Snapshot footer
- [x] Snapshot: Categories + Contents from `/categories` / `/entries`; right-pane tab chrome
- [x] Snapshot right: `/entries/{path}?zahir=1` → Viewer summary + Templates / Metadata / Writing tabs (hide when empty); disk preview still API-limited
- [x] Delta: Added / Modified / Removed → paths (timestamp groups) → Snapshot overview from `/delta`
- [x] Lenses: lens names → member paths → entry detail (`/lenses`, `/lenses/{name}`, Zahir detail)
- [x] Duplicates: groups → member paths → detail (`GET /duplicates`)
- [x] Settings: Global/Local + toggles/steppers/theme + live read-only TOML (`GET`/`PATCH /settings/{scope}`)
- [x] Snapshot Contents `n/total` (shared `PathsPane` footer)
- [x] Feature `ui` + `StaticMount::Dir` / `UBLX_WEB_DIST` (Embedded still TODO)

**Next fill-in (see [`TUI_STRUCTURE.md`](TUI_STRUCTURE.md) checklist):**

- [ ] Catalog search
- [ ] Theme switcher from `Palette` tokens
- [ ] `StaticMount::Embedded` for shipping builds

Nice-to-have after MVP: root switcher, snapshot trigger, health/doctor surface, rich viewers.

---

## Serve wiring (sketch)

Today (`src/cli/serve.rs`):

```rust
panza_run(..., api, StaticMount::None)
```

With `ui`:

```rust
#[cfg(feature = "ui")]
let mount = StaticMount::Embedded(ublx_web::embedded_assets());
#[cfg(not(feature = "ui"))]
let mount = StaticMount::None;

panza_run(..., api, mount)
```

API routes stay registered on the host router and take precedence over the static fallback.

---

## Out of scope

- Svelte / Vite / Next as the **application** host (Tailwind under leptos-shadcn-ui is fine if required)
- Hand-rolled primitive kit instead of leptos-shadcn-ui
- Thaw / Leptonic as the primary widget layer
- Native Rust GUI (egui/iced) as the primary web surface
- Cloud multi-tenant hosting
- TUI viewer parity (previews, enhance, settings editor)
- Reimplementing clap serve / health / static (panza / THI-165)

---

## Done when

Feature-enabled `ublx` can `serve` and browse entries / delta / lenses / health in-browser with at least one real UBLX theme via CSS variables; default (no `ui`) build still has API-only `serve`. Shipped as **v0.2.0**.

---

## Related docs

- TUI layout map (port reference): [`TUI_STRUCTURE.md`](TUI_STRUCTURE.md)
- In-repo CLI notes: [`src/cli/README.md`](../src/cli/README.md)
- Roadmap: [`docs/ROADMAP.md`](ROADMAP.md)
- Public CLI (API today): [ublx.dev CLI — serve](https://ublx.dev/cli#ublx-serve) (web UI section when published)
