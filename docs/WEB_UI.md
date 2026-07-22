# Embedded web UI for `ublx serve` (THI-157 / v0.2.0)

Design note for the optional catalog browser. Implementation has not started; this locks stack and packaging so work can proceed without re-litigating the shape.

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
| Mode chrome | Tabs / navigation from shadcn-ui |
| Entries | Table or dense list + Input / Select for filters |
| Detail | Card or aside panel; preformatted Zahir JSON |
| Routing | `leptos_router` (`/`, `/delta`, `/lenses`, `/health`) |

**TUI → web (roles, not a 3-pane clone):**

| TUI | Web MVP |
| --- | ------- |
| Snapshot list + selection | Entries list/table + detail |
| Categories left rail | Filter controls (not a third column) |
| Delta / Lenses tabs | Router modes |
| Theme / brand | shadcn theme tokens fed from `Palette` |
| Right-pane Viewer | Skip in this issue |

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

Catalog browser — **not** a full TUI port. Dense, path-first (list + detail). Skip file viewers (md/pdf/image) in this issue.

- [ ] App shell (shadcn-ui): theme switcher; connection to local serve (same-origin when embedded)
- [ ] **Entries:** path / category / size; filter (category, size, path text); select row
- [ ] **Detail:** selected entry; optional Zahir JSON (`?zahir=1`)
- [ ] **Delta:** added / mod / removed
- [ ] **Lenses:** list + member paths
- [ ] **Health:** doctor-ish summary from `/health` (and optionally `/doctor`)

Nice-to-have after MVP (not required for Done): root switcher, snapshot trigger, more themes, viewers.

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

- In-repo CLI notes: [`src/cli/README.md`](../src/cli/README.md)
- Roadmap: [`docs/ROADMAP.md`](ROADMAP.md)
- Public CLI (API today): [ublx.dev CLI — serve](https://ublx.dev/cli#ublx-serve) (web UI section when published)
