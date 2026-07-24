# Embedded web UI for `ublx serve` (THI-157 / v0.2.0)

Design note for the optional catalog browser. Stack and packaging are locked below.

**The running TUI is the scaffold.** Placement, chrome roles, hotkeys, and **which `Palette` fields are painted together** come from the TUI — not from shadcn defaults, Tailwind habits, or inventing “pretty” pairings. Layout map: [`TUI_STRUCTURE.md`](TUI_STRUCTURE.md). Style truth: [`src/layout/style/`](../src/layout/style/) (especially [`core.rs`](../src/layout/style/core.rs) `ThemeStyles`). Palette data: [`src/themes/`](../src/themes/).

Work lands as **mini-PRs onto long-lived `dev`**, then a fat PR `dev` → `main` at **v0.2.0**.

**Linear:** [THI-157](https://linear.app/thicclatka/issue/THI-157/web-ui-leptos-feature-flagged-for-ublx-serve-v020)  
**Depends on:** [THI-156](https://linear.app/thicclatka/issue/THI-156/ublx-serve-local-http-api-over-ublx) (Done — JSON API + panza)  
**Target release:** **v0.2.0**

---

## Goal

Optional **embedded catalog browser** for `ublx serve`: the TUI experience in a browser — same chrome, focus model, hotkeys, viewers, and theme personality — not a thin dashboard over JSON.

Default crates.io installs stay **API-only** (`StaticMount::None`). Opt in from a full checkout with `./crates/ublx-web/build.sh` then `cargo install --path . --features ui`. **Homebrew** ships with `--features ui` (Embedded) by default.

---

## Locked decisions

| Decision | Choice | Why |
| -------- | ------ | --- |
| Feature name | **`ui`** | Short; documents as `cargo build --features ui` |
| Framework | **Leptos** (CSR) | All-Rust; embeds cleanly; no Node SPA host |
| Component library | **[leptos-shadcn-ui](https://github.com/cloud-shuttle/leptos-shadcn-ui)** | Real widgets (table, tabs, input, …) for Leptos 0.8+; no hand-rolled CSS kit |
| HTTP shell | **panza** only | No second bind/health/static stack |
| Static mount | `StaticMount::Embedded` when `ui`; `None` otherwise | panza already SPA-falls-back to `index.html` |
| Data path | Same-origin **JSON API** from THI-156 (+ small preview routes as needed) | UI is a client of serve — not Leptos server functions |
| Interaction | Keyboard-first: **arrows + TUI hotkeys** (see keymap) | Mouse remains secondary; parity with TUI |
| Not chosen | Svelte / Vite / Next as the app host; egui-as-web; Thaw; Leptonic | Host stays Leptos CSR + panza; shadcn-ui is the widget layer |

### CSR vs SSR

Prefer **CSR WASM + embed**, not full SSR / `cargo-leptos` dual-compile of the `ublx` binary.

- Serve already owns the Axum router via panza; SSR would fight that shell.
- Localhost catalog browser does not need SEO or first-paint HTML from Rust.
- UI talks to existing routes with `fetch` — matches “same-origin JSON API.”

Dev loop may use `StaticMount::Dir("…/dist")` / `UBLX_WEB_DIST` so assets rebuild without re-embedding every tweak. Shipping builds use **Embedded**.

---

## Cargo / packaging

```toml
[features]
default = ["zahir-netcdf"]           # unchanged — no UI
ui = ["dep:rust-embed"]              # embeds dist/ into the host binary
```

Rules:

- Default binary includes `ublx serve` **API**; **no** Leptos / WASM deps.
- `--features ui` enables embedded assets and switches serve to `StaticMount::Embedded` ([`web_embed.rs`](../src/cli/serve/web_embed.rs)).
- `UBLX_WEB_DIST` overrides to `StaticMount::Dir` for the `mise run web` rebuild loop (no host recompile).
- Do **not** hide API-only serve behind `ui`.
- Workspace crate **`crates/ublx-web/`** is WASM CSR only (`publish = false`). It is **not** a crates.io dependency of `ublx` — that kept `cargo publish` from working with a path-only dep. Host embedding lives in `ublx` via rust-embed of `crates/ublx-web/dist/`.
- crates.io / `cargo install ublx` stays API-only. Embedded UI: git checkout + `build.sh` + `--features ui`, or Homebrew.

### Build story

1. Build CSR assets (`./crates/ublx-web/build.sh` / `mise run web`) → `dist/`.
2. `cargo build --features ui` embeds `dist/` into the binary (`StaticMount::Embedded`).
3. Dev loop: set `UBLX_WEB_DIST=…/crates/ublx-web/dist` (mise `web` does this) so Dir serves fresh assets without re-embedding.
4. `build.sh` also emits `dist/tailwind.css` (no CDN) — needs Node/npm.
5. `cargo publish` packages `ublx` only (default features); does not require or publish `ublx-web`.

---

## Components & layout

**UI kit:** [leptos-shadcn-ui](https://github.com/cloud-shuttle/leptos-shadcn-ui). Prefer published components over inventing primitives.

| Need | Pieces |
| ---- | ------ |
| Mode chrome | Tabs via [`nav`](../crates/ublx-web/src/nav.rs) (`MainMode` + optional `/?mode=`) |
| Lists | Dense path lists + focus + `n/N` (right-aligned) |
| Right pane | Viewer / Templates / Metadata / Writing — full TUI content, not JSON dumps |
| Routing | Stay on `/`; never use API path segments as UI pages |

**TUI → web:** Mirror chrome, **placement**, and **style pairings** from the TUI. Open the TUI style helper for the surface you are porting — do not guess from CSS variable names.

| TUI | Web |
| --- | --- |
| Main tabs + brand | In-app mode tabs + `UBLX` |
| Indexed root gap | Project path under tabs |
| Categories / Contents / Right | 3-pane shell (`ThreePane`) |
| Arrow / hjkl / digit / pane hotkeys | Same actions in browser (ignore when typing in inputs) |
| Metadata / Writing tables | Pretty KV / column-stat tables (TUI renderers’ rules) |
| Markdown / code / image / … | Ported viewers in the Viewer tab |
| Settings | Scope · controls · live read-only TOML (no TOML text editor); default scope **Local** |
| Theme | Same `Palette` fields the TUI paints, exposed as CSS tokens |
| `?` help | Mode-aware sections/bindings — keep [`help.rs`](../crates/ublx-web/src/help.rs) in step with [`src/render/overlays/help.rs`](../src/render/overlays/help.rs) |

---

## Theming

### Hard rule (agents)

1. **Scaffold = TUI.** Before changing web colors or chrome CSS, read [`ThemeStyles`](../src/layout/style/core.rs) (and the render/layout call site). Palettes were authored for those pairings across every shipped theme — Oblivion Ink looking fine is not proof.
2. **CSS vars are transport, not design.** shadcn names (`--primary`, `--secondary`, …) are only a wire format for leptos-shadcn-ui. **Never** assume shadcn’s usual pairing (e.g. “primary text on secondary bg”). That breaks contrast on Resin Record, Archival Simulacra, Silent Sheet, Parched Page, Pale Mirror, Obdurate Noon, Faded Echo, and others.
3. **Copy TUI fg/bg pairs into CSS.** Example — active tab / active tab-node:
   - TUI: `tab_active()` → `fg(tab_active_fg).bg(tab_active_bg)`
   - Web: `color: hsl(var(--secondary-foreground)); background: hsl(var(--secondary));`
   - **Wrong:** `color: hsl(var(--primary))` on `--secondary` (`focused_border` on `tab_active_bg`).
4. **`focused_border` is for focus chrome** (panel border, ring, search underline) — not tab label ink. See TUI panel borders vs `tab_active()`.
5. **Verify more than one theme** (at least one light + one high-contrast dark like Archival / Resin) before calling theming done.

### Token export

[`themes::css`](../src/themes/css.rs) maps [`Palette`](../src/themes/mod.rs) → HSL tokens (`color_to_hsl_token` / `rgb_to_hsl_token`). Settings `theme=` updates the **effective** (global∪local) set; the web client applies `css.vars` on `:root` live.

| TUI style / role | `Palette` field(s) | CSS custom property |
| ---------------- | ------------------ | ------------------- |
| page bg / body text | `background` / `text` | `--background` / `--foreground` |
| `tab_active()` | `tab_active_bg` / `tab_active_fg` | `--secondary` / `--secondary-foreground` |
| `tab_inactive()` bg | `tab_inactive_bg` | `--muted` |
| focused panel border / ring | `focused_border` | `--ring`, `--primary` (focus only) |
| `search_text()` | `search_text` | `--search` |
| toast / notification block | `notification_bg` | `--notification` |
| `hint_text()` | `hint` (+ `popup_bg` in TUI) | `--hint`, `--muted-foreground` |
| popup / help panel | `popup_bg` | `--card`, `--popover`, `--accent` |
| `table_row_style` stripes | `popup_bg` + `adjust_surface_rgb(…, table_stripe_lighten)` | `--card` (even) / `--table-stripe` (odd) |
| `delta_*()` | `delta_added` / `delta_mod` / `delta_removed` | `--delta-*` |
| `title_brand()` | `title_brand` | `--brand` (also favicon “U”) |
| page bg (favicon tile) | `background` | `--background` (favicon square) |
| footer / status pills | `node_pill_background()` | `--node`, `--border`, `--input` |

Favicon (`link[rel=icon]`) is rebuilt on each theme apply: page `background` fill + `title_brand` letter — same fields the TUI uses for brand chrome.

**MVP:** full shipped palette list from Settings. `styles.css` keeps Oblivion Ink only as **pre-fetch fallback**.

API on `GET`/`PATCH /settings/{scope}`:

```json
"css": {
  "name": "Oblivion Ink",
  "appearance": "dark",
  "vars": { "--background": "214 65% 11%", "...": "..." }
}
```

`css` always reflects the **merged** theme (local overrides global), even when editing Global scope.

---

## MVP definition (v0.2.0 Done)

**Not** “JSON browser with tabs.” **Yes** TUI-grade browse:

| Area | Required for Done |
| ---- | ----------------- |
| Chrome | Tabs, path gap, 3-pane, Last Snapshot / catalog search, Settings, mode-aware `?` help overlay |
| Keyboard | Arrows + TUI hotkeys for focus, list move, mode switch, right-pane tabs, sort, search, find, `?` help (where applicable) — see [`src/ui/keymap.rs`](../src/ui/keymap.rs) |
| Lists | Snapshot / Delta / Lenses / Duplicates with `n/N` **bottom-right**; middle sort node where TUI has it |
| Selection / menus | Multi-select (contents), Space quick-actions / context menu, Command Mode overlay — TUI parity for browse+act |
| Right pane | Viewer body + Templates / Metadata / Writing |
| Metadata / Writing | Pretty tables (KV + typed column stats), not raw pretty-JSON only |
| Viewers | Markdown, syntect/code, images, tables/CSV, and the other TUI viewer families that do not need a local GUI tool; PDF/video via same optional-tool story or honest fallback |
| Theme | `Palette` → CSS tokens; Settings theme control applies them |
| Ship | `StaticMount::Embedded` for `--features ui` |

Mouse click remains supported; keyboard is first-class.

**Explicitly after MVP (still fine as follow-ons on `dev`):** enhance-from-UI polish beyond Command/Space paths, fullscreen viewer polish, root switcher / snapshot trigger / doctor surfaces — unless a mini-PR lands them early.

---

## Landed on `dev` (shell)

- [x] App shell: main tabs, brand, project path, Last Snapshot footer
- [x] Snapshot / Delta / Lenses / Duplicates / Settings modes (API-backed)
- [x] Right-pane tab chrome + Zahir section split (Templates / Metadata / Writing) — **content still thin**
- [x] Contents `n/N` bottom-right (`PathsPane`)
- [x] Catalog search (`/` strip + Skim fuzzy)
- [x] Settings controls + live read-only TOML; `GET`/`PATCH /settings/{scope}`; `GET /duplicates`
- [x] Feature `ui` + Embedded (`crates/ublx-web/dist/` via rust-embed); Dir via `UBLX_WEB_DIST` for `mise run web`
- [x] Keyboard focus + hotkeys (digits/`~`/hjkl/arrows/`g``G`/Tab/`vtmw`/Shift+Tab/`s` sort)
- [x] Help overlay (`?`) + footer `? — Help` chip; 7px shell inset from browser edge
- [x] Palette → CSS tokens (`themes::css`); Settings theme dropdown applies live
- [x] Middle sort node left of `n/N` (Snapshot / Dupes / Delta) + `s` cycle
- [x] Pretty Metadata + Writing (KV / column-stat tables; `typed_column_tables`)
- [x] Markdown viewer (Viewer tab; `/content/{*path}`)
- [x] Code / syntect viewer (JSON/TOML/YAML/XML/HTML/INI/Log/Code; per-palette `.tmTheme`)
- [x] Tables / CSV (+ misc text)
- [x] Images / SVG (+ Audio/Epub embedded covers)
- [x] PDF / video tool-backed previews (Poppler/MuPDF / ffmpeg; honest missing-tool errors)
- [x] Viewer find (Shift+S strip; Enter / `n`/`N` / Esc)
- [x] Multi-select (Ctrl+Space; Space toggle; Snapshot / Lenses)
- [x] Space / context menu (Open / Copy / Ignore + rename/delete/lens/enhance via serve `/fs` + `/lenses`)

---

## Mini-PR plan onto `dev`

One concern per PR. Order is dependency-aware; titles are suggestions.

### Hard rule — `?` help stays in lockstep

Every mini-PR that adds or changes a **keybinding, selection model, overlay, or Viewer affordance** must also update the web `?` help overlay in the **same PR** — same spirit as the TUI (`src/render/overlays/help.rs`):

- Add / adjust rows in [`crates/ublx-web/src/help.rs`](../crates/ublx-web/src/help.rs) for the modes that gain the feature (General / Viewer / Multi-select / QA / Settings / …).
- Match TUI section placement and wording where the binding exists in both; omit TUI-only actions that do not work over serve.
- Do **not** leave stale footnotes (“lands in a later PR”) once the feature ships.
- Mini-PR numbers in docs/comments: write `mini-PR 13`, never bare `#13` (GitHub auto-links issue numbers).

| # | PR onto `dev` | Delivers | Notes / anchors |
| - | ------------- | -------- | --------------- |
| **1** | **Keyboard focus + hotkeys** | ✅ Landed (#43) | [`keys.rs`](../crates/ublx-web/src/keys.rs) + [`focus.rs`](../crates/ublx-web/src/focus.rs) |
| **2** | **Help overlay (`?`)** | ✅ Landed (#44) — mode-aware popup, footer chip, Esc/`?`/backdrop close | [`help.rs`](../crates/ublx-web/src/help.rs) |
| **3** | **Palette → CSS tokens** | ✅ Landed — `Palette` → HSL vars; Settings theme switches live look | [`themes/css.rs`](../src/themes/css.rs); WEB_UI token table above |
| **4** | **Middle sort node** | ✅ Landed — sort left of `n/N` + `s` cycle (TUI `ContentSort` rules) | [`sort.rs`](../crates/ublx-web/src/sort.rs); [`middle.rs`](../src/render/panes/middle.rs) |
| **5** | **Pretty Metadata + Writing** | ✅ Landed (#47) — host `SectionView` + Settings `typed_column_tables` | [`export.rs`](../src/render/kv_tables/export.rs); [`kv_tables.rs`](../crates/ublx-web/src/kv_tables.rs) |
| **6** | **Markdown viewer** | ✅ Landed (#49) — host HTML via `/content/{*path}` | [`viewer.rs`](../crates/ublx-web/src/viewer.rs); [`render/viewers/markdown/`](../src/render/viewers/markdown/) |
| **7** | **Code / syntect viewer** | ✅ Landed — syntect HTML for code cats via `/content`; per-palette `.tmTheme` in `assets/syntect-themes/` (THI-170) | [`syntect_text`](../src/render/viewers/syntect_text.rs); [`themes/syntect.rs`](../src/themes/syntect.rs); `/content?theme=` for live picker |
| **8** | **Tables / CSV (+ misc text)** | ✅ Landed — host HTML table / `<pre>` via `/content` | [`csv_handler`](../src/render/viewers/csv_handler.rs), pretty tables |
| **9** | **Images (and covers)** | ✅ Landed — raster/SVG via `/content?format=raw`; Audio/Epub covers via `?format=cover` | [`viewer.rs`](../crates/ublx-web/src/viewer.rs); [`serve.rs`](../src/cli/serve.rs) `/content` |
| **10** | **PDF / video / tool-backed** | ✅ Landed — PDF/video PNG via `/content?format=raw`; web Shift+J/K/B/E = preview scroll (TUI) or PDF pages when a PDF is open; `Page n / N` footer; tool-missing under `<img>` | [`pdf_preview`](../src/render/viewers/pdf_preview.rs), [`video_preview`](../src/render/viewers/video_preview.rs); [`viewer.rs`](../crates/ublx-web/src/viewer.rs) |
| **11** | **Viewer find** | ✅ Landed — Shift+S find strip on right `title_bottom`; Enter / `n`/`N` / Esc; DOM marks | [`viewer_find.rs`](../crates/ublx-web/src/viewer_find.rs) |
| **12** | **Preview / file body API** | ✅ Landed (#59) — windowed `/content` (`offset`/`limit`); stub `EXT file` labels; CSV pinned header; Metadata sticky headers; collapsible directory/schema trees + Expand/Collapse; Epub/Audio cover Viewer | [`serve.rs`](../src/cli/serve.rs) `/content`; [`viewer.rs`](../crates/ublx-web/src/viewer.rs); [`schema.rs`](../src/render/kv_tables/schema.rs) |
| **13** | **Multi-select** | ✅ Landed — Ctrl+Space enter/exit; Space toggle rows on Snapshot / Lenses contents (not Dupes); █ chrome + `n/N · k sel`; **`?` Multi-select section** | [`multiselect.rs`](../crates/ublx-web/src/multiselect.rs); TUI [`ui/multiselect.rs`](../src/ui/multiselect.rs) |
| **14** | **Space / context menu** | ✅ Landed — Space QA + `a` bulk; serve `/fs/*` + lens writes; confirm / rename / lens picker; **`?` QA rows** | [`space_menu/`](../crates/ublx-web/src/space_menu/); [`serve/fs.rs`](../src/cli/serve/fs.rs); TUI [`ui/menus/`](../src/ui/menus/) |
| **15** | **Command Mode** | ✅ Landed — Ctrl+a chord + menu; d/t/s/r/x/l/p; theme/root pickers; serve `/export/*`; **`?` Command section** | [`command_mode/`](../crates/ublx-web/src/command_mode/); [`serve/export.rs`](../src/cli/serve/export.rs); TUI [`ui/ctrl_chord.rs`](../src/ui/ctrl_chord.rs) |
| **16** | **`StaticMount::Embedded`** | ✅ Landed — `--features ui` embeds `dist/`; `UBLX_WEB_DIST` keeps Dir for `mise run web` | [`web_embed.rs`](../src/cli/serve/web_embed.rs); [`serve/mod.rs`](../src/cli/serve/mod.rs) `static_mount` |

**Ops / chrome follow-ups** (separate PRs after or interleaved when small):

| PR | Delivers |
| -- | -------- |
| Toast polish | Shared stack (bottom-right, max 3) via shadcn `Toast`; Space / Command Mode wired |
| Root switcher | Click project path (or Command Mode `p`) → `GET`/`PUT /roots/current` |
| Snapshot trigger | UI for `POST`/`GET /snapshot` |
| Doctor / health surface | `GET /doctor` + panza health |

**Post v0.2.0** (tracked in Linear, not MVP blockers):

- Catalog fetch cache across main-tab switches — [THI-168](https://linear.app/thicclatka/issue/THI-168/web-ui-cache-catalog-fetches-across-main-tab-switches-v021)
- Font selection (CSS `--font-mono` / Settings) — [THI-169](https://linear.app/thicclatka/issue/THI-169/web-ui-font-selection-post-v020)

Do **not** expand a mini-PR into “finish the whole Viewer stack” — keep each PR reviewable. Do **include** the matching `?` help rows in that same PR when keys or overlays change.

---

## Serve wiring

```rust
#[cfg(feature = "ui")]
let mount = if let Some(dir) = std::env::var_os("UBLX_WEB_DIST") {
    StaticMount::Dir(dir.into())
} else {
    StaticMount::Embedded(web_embed::embedded_assets()) // serve/web_embed.rs
};

#[cfg(not(feature = "ui"))]
let mount = StaticMount::None;

panza_run(..., api, mount)
```

API routes stay on the host router and take precedence over the static SPA fallback.

---

## Out of scope (product)

- Svelte / Vite / Next as the **application** host (Tailwind under leptos-shadcn-ui is fine)
- Hand-rolled primitive kit instead of leptos-shadcn-ui
- Thaw / Leptonic as the primary widget layer
- Native Rust GUI (egui/iced) as the primary web surface
- Cloud multi-tenant hosting
- Reimplementing clap serve / health / static (panza)

**In scope for v0.2.0 MVP:** keyboard parity, multi-select + Space menu + Command Mode, pretty Metadata/Writing, and the viewer families listed above — not “JSON in a `<pre>` forever.”

---

## Done when

Feature-enabled `ublx serve` is a **keyboard-usable TUI-shaped browser**: modes, search, hotkeys, multi-select / Space actions / Command Mode, pretty Zahir tables, and real Viewer content (md/code/tables/images/…); themes from `Palette`; Embedded ship path works. Default (no `ui`) build stays API-only. Shipped as **v0.2.0**.

---

## Related docs

- TUI layout map (port reference): [`TUI_STRUCTURE.md`](TUI_STRUCTURE.md)
- In-repo CLI notes: [`src/cli/README.md`](../src/cli/README.md)
- Roadmap: [`docs/ROADMAP.md`](ROADMAP.md)
- Public CLI: [ublx.dev CLI — serve](https://ublx.dev/cli#ublx-serve)
