# UBLX roadmap

Living backlog for **UBLX** (TUI catalog browser). Not a release promise — prioritize by profiling, user need, and architectural fit.

**Status (v0.2.x):** Index → SQLite → Snapshot / Delta / Lenses / Duplicates / Settings; ZahirScan enrichment; strong Viewer (markdown, tables, images, PDF/video via optional tools, syntect code, Zarr, `.tet`). Config is TOML with hot reload. Headless catalog CLI: `ublx query` / `ublx doctor` / `ublx serve` (JSON API via panza); remote `--url` / `UBLX_URL` on query+doctor. Optional embedded web UI (`--features ui`, THI-157) — `cargo install ublx --features ui` embeds `assets/web-ui/` from the crates.io tarball; Homebrew / source `build.sh` same path. No plugin system, Lua, in-TUI runner, or user-authored themes yet.

Track work in GitHub Issues — **parent** issues by category, **sub-issues** for concrete tasks:

| Category              | Parent                                                    |
| --------------------- | --------------------------------------------------------- |
| Platform & extensions | [#5](https://github.com/Latka-Industries/UBLX/issues/5)   |
| Config & scripting    | [#6](https://github.com/Latka-Industries/UBLX/issues/6)   |
| Themes                | [#7](https://github.com/Latka-Industries/UBLX/issues/7)   |
| Viewer & code         | [#8](https://github.com/Latka-Industries/UBLX/issues/8)   |
| Lenses                | [#9](https://github.com/Latka-Industries/UBLX/issues/9)   |
| Performance & scale   | [#10](https://github.com/Latka-Industries/UBLX/issues/10) |
| Maintenance & docs    | [#11](https://github.com/Latka-Industries/UBLX/issues/11) |

---

## 0. Headless catalog CLI

**Goal:** Read (and diagnose) the `.ublx` SQLite catalog without the TUI — agents, scripts, piping to `jq`.

| Item | Status | Notes |
| ---- | ------ | ----- |
| Clap subcommands + shared catalog open | Done (THI-152) | `query` / `doctor`; `-s`/`-f`/`-x` unchanged |
| `ublx query` | Done (THI-153) | List/filter/detail/delta/lenses; `--json`; nested zahir |
| `ublx doctor` | Done (THI-154) | PASS/WARN/FAIL; `--fix`; blocked while snapshot writing unless `--force` |
| `ublx serve` | Done (THI-156 / v0.1.13) | Local HTTP API via panza (`StaticMount::None`) |
| Remote `--url` / `UBLX_URL` | Done (THI-167 / v0.1.14) | `query` / `doctor` against a running serve |
| Web UI for serve | In progress (THI-157 / v0.2.0) | Shell on `dev`; MVP = TUI-grade browse+act (hotkeys, help, multi-select, Space menu, Command Mode, viewers) — mini-PR plan in [WEB_UI.md](WEB_UI.md), layout in [TUI_STRUCTURE.md](TUI_STRUCTURE.md) |
| Crate split (catalog vs TUI) | Backlog (THI-155) | Faster compiles for CLI iteration |

Parent: [THI-151](https://linear.app/thicclatka/issue/THI-151).

---

## 1. Platform & extensions

**Goal:** Decide how third-party behavior enters UBLX without forking the binary.

| Item                                     | Notes                                                                                                                                                 |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Plugin system — design spike             | Scope: viewer extensions, index hooks, lens exporters, or CLI-only? Options: Rust dylibs, WASM, external CLI contract (like `tree` / `ffmpeg` today). |
| Extension contract for optional binaries | Document discovery, Settings surfacing, graceful fallback; extend the existing PATH-probe pattern.                                                    |

**Depends on:** ADR before large bets (Lua hooks, theme files, custom exporters).

---

## 2. Config & scripting

**Goal:** Clearer power-user control without breaking TOML validation and hot reload.

| Item                             | Notes                                                                                                                      |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Lua (or scripting) — feasibility | Today: global + local `ublx.toml` only. Clarify: generate config, replace config, or runtime hooks (snapshot / open file). |
| Config-driven viewer thresholds  | Expose or tune caps currently in `VIEWER_TEXT_CACHE` (CSV/markdown/syntect min bytes, truncation).                         |
| Snippets / “insert” in Viewer    | Separate from config if the goal is templates in preview, not `ublx.toml`.                                                 |

---

## 3. Themes

**Goal:** Beyond the fixed palette list in `src/themes/palettes.rs`.

| Item                       | Notes                                                                                                                |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Theme maker (MVP)          | Export or duplicate a palette; write `theme = "..."` to local config (selector already persists choice).             |
| User-defined themes (full) | Persist custom `Palette` fields (file or config); may require moving palettes out of compile-time-only `ALL_THEMES`. |

---

## 4. Viewer & code

**Goal:** Preview quality and optional execution without turning UBLX into an IDE.

| Item                       | Notes                                                                                                                                           |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Syntax highlighting — ADR  | **Current:** syntect + `sublime_syntaxes`; each [`Palette`](../src/themes/palettes.rs) points at a first-pass `.tmTheme` in [`assets/syntect-themes/`](../assets/syntect-themes/) (`Palette.syntect`). **Alternative:** tree-sitter (structure-aware, heavier deps). |
| In-TUI code runner         | **Current:** Open (Terminal) / Open (GUI) via `editor_path` / `$EDITOR`. Runner needs sandbox, cwd, output surface.                             |
| Grammar / highlight polish | Token colors in the per-palette `.tmTheme`s; more grammars / path→grammar mapping if ADR keeps syntect.                                          |

---

## 5. Lenses

**Goal:** Lenses as durable “focused lists” with richer context, not only path playlists.

| Item                        | Notes                                                                                              |
| --------------------------- | -------------------------------------------------------------------------------------------------- |
| Lens notes / description    | Schema today: `lens(id, name)` + `lens_path`; no per-lens or per-path notes.                       |
| Richer lens Markdown export | Export today: `# title` + links; extend with category, size, Zahir snippets, writing stats, notes. |
| Lens workflows              | Reorder paths, sort/filter within lens, duplicate lens, import from markdown list.                 |

Module CRUD and export (`Ctrl+A` `l`) already exist; see `src/modules/lenses.rs` and `src/engine/db_ops/lens_export.rs`.

---

## 6. Performance & scale

**Goal:** Stable RSS when switching large previews; honest behavior on huge files.

Engineering notes also live in local `TODO.md` (gitignored); items below are the tracked issue set.

| Item                                    | Notes                                                                                   |
| --------------------------------------- | --------------------------------------------------------------------------------------- |
| Evict viewer caches on selection change | `viewer_text_cache`, async viewer state, stale `Arc<str>`.                              |
| Large files — streaming / windowing     | Head+tail or chunked read with explicit “truncated” label vs full buffer in memory.     |
| Image / PDF cache eviction              | Avoid retaining multiple large rasters when only one pane is visible.                   |
| Regression checks                       | Profile row-switch between max-sized previews; RSS should not grow linearly per switch. |

---

## 7. Maintenance & docs

| Item                                  | Notes                                          |
| ------------------------------------- | ---------------------------------------------- |
| Keep this roadmap in sync with issues | Update when closing or reprioritizing parents. |

---

## Suggested sequencing

1. **Optional web UI for serve** — ✅ **v0.2.x** Leptos CSR + `--features ui` (THI-157); crates.io `cargo install --features ui` embeds `assets/web-ui/` ([WEB_UI.md](WEB_UI.md)).
2. **Lenses** — notes + export (user-visible, low architectural risk).
3. **Performance** — memory / large-file hardening.
4. **Platform ADR** — plugins / extension contract before Lua, runner, user themes.
5. **Viewer ADR** — syntect vs tree-sitter; then runner or grammar work.
6. **Themes / config scripting** — after persistence model is clear.

---

## Out of scope (for this repo)

- General-purpose file manager (see [yazi](https://github.com/sxyazi/yazi) in README).
