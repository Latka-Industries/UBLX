# UBLX

[![Crates.io](https://img.shields.io/crates/v/ublx.svg)](https://crates.io/crates/ublx)
[![docs.rs](https://img.shields.io/docsrs/ublx)](https://docs.rs/ublx)
![Build](https://github.com/thicclatka/ublx/workflows/Build/badge.svg)
![Rust](https://img.shields.io/badge/rust-1.95-orange.svg)

[\_Ublx ... Safe when taken as directed.][ubik]

UBLX is a **TUI that turns any directory into a flat, navigable catalog** — categories, previews, metadata, and templates in the terminal. (Driven by [nefaxer] and [zahirscan])

**_Currently in development, expect breaking changes._**

## Before you start

- a terminal emulator with **truecolor** (24-bit color) & image support
- Strongly recommend using a terminal with a [**Nerd Font**][nerd-fonts]

**Optional**:

| Tool                   | Role                                                                                    |
| ---------------------- | --------------------------------------------------------------------------------------- |
| `tree`                 | Directory preview in the Viewer when on `PATH`                                          |
| `pdftoppm` or `mutool` | PDF page raster preview                                                                 |
| `ffmpeg`               | Video frame preview                                                                     |
| `netcdf`/`libnetcdf`   | Default features link to NetCDF. To build without: `cargo build --no-default-features`. |

## Install

### Homebrew

```bash
brew tap thicclatka/ublx https://github.com/thicclatka/ublx
brew install thicclatka/ublx/ublx
```

### Cargo

```bash
cargo install ublx

# or compile from source
cargo build --release

# compile without netcdf
cargo build --release --no-default-features
```

## What it does

- **Index once, then browse** — One run updates the catalog; the prior index enables fast diffs. Per-root SQLite under your user cache (`ubli/`; stem is sanitized dir name plus path hash; extension matches the `ublx` package name). Config file names and paths: [Configuration](#configuration).
- **Path catalog vs full metadata** — By default, indexing records **paths and filetype/category** hints (fast, lightweight). **[zahirscan]** adds the rich **Zahir JSON** used for deep previews, Templates, Writing stats, and Metadata tables. Batch that with `enable_enhance_all`, scope it with **`[[enhance_policy]]`**, or run **Enhance with ZahirScan** on demand (quick actions / multi-select). Keys and behavior: [Configuration](#configuration).
- **TUI** — Main tabs (left to right when all are shown): **Snapshot** | **Lenses** | **Delta** | **Duplicates** | **Settings** — **Lenses** and **Duplicates** appear only when the DB has lenses or duplicate groups. Three-pane layout and keys: [Modes](#modes) and [Panes overview](#panes-overview). Search across & within files, vim motions, theme selector, quick actions menu, command mode, toast notifications, fullscreen toggle.
- **Snapshot-only** — Index without the TUI (`-s` / `--snapshot-only`; [Usage](#usage)).
- **Export** — Pretty-printed Zahir JSON ([Usage](#usage)); Lenses converted to markdown.

## Use case

Looking for a file manager? Use [yazi] for that; where UBLX comes in:

- Best for navigating and handling project directories that you frequent
- Preview & extract filetype specific metadata without having to open a file
- **For large directories**: can still get a quick understanding of filetypes contained without metadata enhancement

## Modes

| Tab            | Description                                                                                                                                                                                                                                       |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Snapshot**   | From the current index: categories and file list; right pane shows Templates / Viewer / Metadata / Writing for the selected row.                                                                                                                  |
| **Delta**      | Added / Mod / Removed since last snapshot; same 3-pane layout with overview in the right pane.                                                                                                                                                    |
| **Lenses**     | Saved lists of items with a specific focus (e.g. a “lens” on a subset of files); left = lens names, middle = paths in the selected lens. Shown when the DB has at least one lens.                                                                 |
| **Duplicates** | Groups of duplicate files by content hash; left = group names, middle = paths in the group. Run duplicate detection from [Command Mode](#context-menu-multi-select--command-mode) to populate; tab appears when groups exist.                     |
| **Settings**   | Global vs local `ublx.toml`: theme, layout, `bg_opacity`, bool toggles (e.g. `show_hidden_files`, `run_snapshot_on_startup`), **`typed_column_tables`** (`none` / `abbrev` / `full`), and more; **e** opens the active scope's file in `$EDITOR`. |

Cycle main tabs with `~`.

## Panes overview

The right pane shows Viewer, Templates, Metadata, or Writing for the selected item. **Tab** switches focus between the left (categories) and middle (contents) panes; **h** / **l** also focus left or middle.

- **Focus a right-pane tab** — **v** (Viewer), **t** (Templates), **m** (Metadata), **w** (Writing). **Shift+Tab** cycles through right-pane tabs.
- **Scroll in the preview** — **Shift+b** / **Shift+e** (jump to top/bottom), **Shift+↑** / **Shift+↓** or **Shift+K** / **Shift+J** (line by line).
- **Viewer fullscreen** — **Shift+F** toggles fullscreen for the Viewer tab.
- **Viewer search** — **Shift+S** opens literal in-pane search in the preview (see in-app help for n/N and Esc).
- **Catalog search** — **/** fuzzy-filters the category and content lists (left and middle) by path/category. Press **Esc** to clear filter.

| Tab           | Content                                                                                                                                                                                                                                                                                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Viewer**    | Previews for the selected file — details in [Viewer Previews](#viewer-previews). Footer: size and last-modified when available.                                                                                                                                                                                                       |
| **Templates** | Extracted template/structure snippet (e.g. document outline) when [zahirscan] provides it.                                                                                                                                                                                                                                            |
| **Metadata**  | Enrichment metadata as **tables**: key/value pairs, and for supported types from [ZahirScan metadata extraction by format][zahirscan-formats]. Typed column-stat tables (String/Number/…) honor [`typed_column_tables`](#configuration). Sections are parsed from the stored [zahirscan] result and rendered with headers and scroll. |
| **Writing**   | **Writing stats** (writing footprint): word count, character counts, and similar stats when [zahirscan] has computed them. Shown in the same table layout as Metadata.                                                                                                                                                                |

### Viewer Previews

- **Markdown** — formatted preview (headings, lists, code blocks, tables inside the doc).
- **CSV-style files** — for `.csv`, `.tsv`, `.tab`, `.psv`, render a pretty table when parsed width is 30 columns or fewer; for wider files, render a structured delimited fallback that shows only viewport-fitting leading columns with row/column truncation counts (still keyed off index type or path match when category labels are off).
- **Images** — terminal preview via [ratatui-image] (downscaled for the pane; larger files may decode off the UI thread; recent previews cached for quick navigation).
- **Zarr directory (`.zarr`)** — show a directory tree of the store
- **[tetration] (`.tet`)** — mmap catalog summary in the Viewer (same text as `tet info --all -n 0`: layout, execution, datasets, chunks, history); works from path even before ZahirScan enhance.
- **Code and structured text** — [syntect] highlighting via [`sublime_syntaxes`][sublime-syntaxes]; grammar from path/extension; colors match theme light/dark. Large buffers are cached for smooth scrolling.
- **Other text** — raw text (length-capped).
- **Binaries** — short label instead of dumping bytes.
- **Directories** — `tree` when available.

## Context Menu, Multi-select, & Command Mode

| Feature                          | Keys           | Notes                                                                                                                                                                                              |
| -------------------------------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Quick actions** (context menu) | **Space**      | Actions for the current row: open, reveal in folder, enhance / lens / copy / rename / delete — depends on tab and row type.                                                                        |
| **Multi-select**                 | **Ctrl+Space** | Toggles multi-select on the **contents** pane (**Snapshot** or **Lenses** only). While on: **Space** toggles selection on the current row; **a** opens the bulk menu; **Esc** leaves multi-select. |
| **Command Mode**                 | **Ctrl+A**     | “Leader” — press a second key for a global action, or wait briefly to see a centered menu listing the letters below. Not available while the catalog search bar is active.                         |

**Command Mode** second key (after **Ctrl+A**):

| Key   | Action                                                   |
| ----- | -------------------------------------------------------- |
| **d** | Run duplicate detection (background)                     |
| **t** | Theme selector (writes theme to local config on confirm) |
| **s** | Take snapshot (background)                               |
| **r** | Reload config from disk                                  |
| **x** | Export Zahir JSON to `ublx-export/`                      |
| **l** | Export lenses as Markdown to `ublx-lenses/`              |
| **p** | Switch indexed project (recents / `ubli/` roots)         |

Press **?** to open the full keybinding help.

## Configuration

Config is optional. If present, **global** config is applied first, then **local** overrides from the indexed directory.

| Platform      | Global config              | Config Cache                   |
| ------------- | -------------------------- | ------------------------------ |
| macOS / Linux | `~/.config/ublx/ublx.toml` | `~/.local/share/ublx/configs/` |
| Windows       | `%APPDATA%\ublx\ublx.toml` | `%LOCALAPPDATA%\ublx\configs\` |

**Local** config (same on all platforms): `.ublx.toml` (default) or `ublx.toml` in the directory you index. Only keys present in each file override defaults.

**Global-only keys**:

| Key                       | Type              | Allowable values / notes                                                                                                                                                                     |
| ------------------------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `opacity_format`          | string (optional) | When `bg_opacity` &lt; 1: OSC 11 payload style — `rgba` (default) or `hex8` (`#RRGGBBAA`). Some terminals prefer one or the other.                                                           |
| `ask_enhance_on_new_root` | bool (optional)   | If `false`, skip the first-run “Enhance all files?” prompt and apply `enable_enhance_all` from config immediately. If `true` or omitted (default), show the prompt for a newly indexed root. |

**Configurable keys** (global and/or local unless noted):

| Key                       | Type                 | Allowable values / notes                                                                                                                                                                                                                                   |
| ------------------------- | -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `theme`                   | string               | See [Themes](src/themes/README.md#allowable-values).                                                                                                                                                                                                       |
| `layout`                  | table                | Pane widths: `left_pct`, `middle_pct`, `right_pct` (each 0–100; must sum to 100). Default: `left_pct = 10`, `middle_pct = 30`, `right_pct = 60`.                                                                                                           |
| `bg_opacity`              | float (optional)     | Page background opacity `0.0`–`1.0`: main pane uses terminal default fill (OSC 11) below `1.0` so wallpaper can show through; omitted or `1.0` = solid theme background. Adjustable in Settings.                                                           |
| `show_hidden_files`       | bool                 | If `true`, include hidden files (e.g. `.*`) in the index.                                                                                                                                                                                                  |
| `hash`                    | bool                 | If `true`, compute blake3 hash per file (slower; used for duplicate detection and change detection).                                                                                                                                                       |
| `exclude`                 | array of strings     | Extra path patterns to exclude from indexing (startup only; not hot-reloadable).                                                                                                                                                                           |
| `editor_path`             | string               | Path to editor for “Open (Terminal)” (e.g. `"vim"`, `"nvim"`). When unset, uses `$EDITOR`.                                                                                                                                                                 |
| `enable_enhance_all`      | bool                 | If `true`, full metadata for all files on snapshot. If `false` (default), only get path plus file-type/category until **Enhance with ZahirScan** per file.                                                                                                 |
| `run_snapshot_on_startup` | bool (optional)      | Default `true` (omit = true): spawn a **background snapshot** when the TUI opens (unless first-run defers it) and when **switching indexed roots** in-session. Set `false` to skip those automatic runs and use the existing DB until you take a snapshot. |
| `typed_column_tables`     | string (optional)    | Metadata/Writing typed column-stat tables from compact `columns` JSON: `none` (hide), `abbrev` (default; cap tables with more than 20 rows to 20), or `full`. Legacy alias: `column_stats`. Adjustable in Settings (Global/Local). Hot-reloadable.         |
| `[[enhance_policy]]`      | TOML array of tables | Optional per-subtree rules (see below). Hot-reloadable with the rest of the overlay.                                                                                                                                                                       |

All of the above except **`exclude`** are **hot-reloadable** when set in a file that UBLX loads (global and/or local). The **global-only** keys in the previous table are hot-reloadable only via **global** config edits, not from local project files.

**`[[enhance_policy]]`** — Each row has `path` (relative to the indexed directory, `/` separators, e.g. `src` or `photos/2024`) and `policy`:

- **`auto`** — ZahirScan runs for files under that prefix when you take a snapshot (same idea as `enable_enhance_all = true`, but only for this subtree).
- **`manual`** — No batch Zahir on snapshot for that subtree (same idea as `enable_enhance_all = false`): the catalog is path-only there until you **Enhance with ZahirScan** on specific files.

The **longest** `path` prefix that matches a file wins. If no row matches, **`enable_enhance_all`** applies globally.

**Live reload** — A file watcher monitors **both** the indexed directory’s local config and the **global** config path; saving either file triggers a debounced reload. A successful parse applies the new merged settings immediately, and the valid merged overlay is written to the per-directory config cache. If the file is invalid, an error is shown and the last successful config saved in cache is used.

## Usage

```text
Usage: ublx [OPTIONS] <DIR>

Arguments:
  <DIR>  Directory to index (default: current directory)

Options:
  -s, --snapshot-only  Headless snapshot. Writes a local config file when this dir has none
  -e, --enhance-all    With `--snapshot-only`: set `enable_enhance_all = true` in new local config and use it for this run
  -f, --full-snapshot  Same as `--snapshot-only --enhance-all`
  -x, --export         Headless: write each Zahir JSON to `ublx-export/` as flat `{path}.json` files. Recommended to run with `--full-snapshot` to get most complete & recent results. Adjust enhance policy in config to fine-tune which paths get ZahirScan
      --dev            Dev mode: tui-logger drain + `move_events` + trace-level default filter
      --themes         Print available themes grouped by appearance
  -V, --version        Print version
  -h, --help           Print help
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).

[nefaxer]: https://github.com/thicclatka/nefaxer
[nerd-fonts]: https://github.com/ryanoasis/nerd-fonts
[ratatui-image]: https://github.com/ratatui-org/ratatui-image
[sublime-syntaxes]: https://crates.io/crates/sublime_syntaxes
[syntect]: https://github.com/trishume/syntect
[tetration]: https://github.com/thicclatka/tetration
[ubik]: https://bookshop.org/p/books/ubik-philip-k-dick/1fc432e3ade32290
[yazi]: https://github.com/sxyazi/yazi
[zahirscan]: https://github.com/thicclatka/zahirscan
[zahirscan-formats]: https://github.com/thicclatka/zahirscan#metadata-extraction-by-format
