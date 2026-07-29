# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-07-29

### Added

- Opt-in Cargo features: default stays TUI + `query`/`doctor`; `serve` and `ui` are optional (`ui` implies `serve`)
- Compile-out TUI so serve-only installs can build without ratatui
- Extract shared catalog paths, SQLite ops, and open/read into workspace crate `ublx-catalog` (published separately)

### Fixed

- Unify catalog list / SQL window helpers for TUI and serve
- Web: Ctrl+↑/↓ jump in Snapshot instead of opening Command Mode (leader matched `ArrowUp`/`ArrowDown`)

## [0.2.5] - 2026-07-28

### Added

- Serve: windowed `GET /entries` with `limit` / `offset` (and server-side filter) for large catalogs
- Web: windowed Snapshot `/entries` fetch
- Web: TUI-parity path marquee, strip clear ×, Shift+F

## [0.2.4] - 2026-07-28

### Added

- TUI: structured Templates tab with inline examples
- Web: expandable Templates accordion

### Fixed

- Web: virtualize Contents / Paths pane for large catalogs

## [0.2.3] - 2026-07-27

### Added

- Web: cache catalog fetches across main-tab switches
- Web: switch project root without a full page reload
- Web: scoped CatalogRefresh invalidation
- Web: soft-boot from `sessionStorage` catalog flags

## [0.2.2] - 2026-07-24

### Added

- Serve: cold-start auto-snapshot when no catalog exists yet

## [0.2.1] - 2026-07-24

### Fixed

- Ship web UI assets in the crates.io package so `cargo install ublx --features ui` works

## [0.2.0] - 2026-07-24

### Added

- Embedded Leptos SPA for `ublx serve` (TUI-parity browse UI): Snapshot / Delta / Lenses modes, search, hotkeys, `?` help, palette → CSS tokens, sort, Command Mode, theme picker, multi-select, Space/bulk menus
- Right-pane viewers on the web: Pretty Metadata, Writing, Markdown, code (syntect), tables/CSV, images/covers, PDF/video, find (Shift+S), code wrap
- Windowed content lists, frozen headers, collapsible trees
- `StaticMount::Embedded` + offline Tailwind; Homebrew formula builds with `--features ui`
- Per-palette syntect theme keys

### Changed

- Split serve / layout / config / web API modules; relocate unit tests under `src/`

### Fixed

- Web: scroll selected row into view on arrow navigation

## [0.1.14] - 2026-07-22

### Added

- CLI: `--url` / `UBLX_URL` remote client for `query` / `doctor` against a running serve

## [0.1.13] - 2026-07-22

### Added

- CLI: `ublx serve` HTTP API for catalog browse and mutations

## [0.1.12] - 2026-07-21

### Added

- Settings: configurable Command Mode leader key

## [0.1.11] - 2026-07-21

### Added

- CLI: `ublx query` and `ublx doctor` for headless catalog inspect / cleanup

## [0.1.10] - 2026-07-20

### Added

- Viewer: SVG preview via resvg

## [0.1.9] - 2026-06-04

### Fixed

- Evict viewer caches on row change; drop image rasters when leaving a row
- Windowed large-file preview and performance regression guards

### Changed

- CI: rust-cache instead of a manual Cargo cache; drop redundant build job

## [0.1.8] - 2026-06-03

### Changed

- Docs site links, issue templates, and stack dependency bumps

## [0.1.7] - 2026-05-27

### Added

- ZahirScan 0.3.3 integration: typed column tables, column-stats config, Settings UI rename
- Viewer: `.tet` info catalog for Tetration files

### Changed

- Move project under Latka-Industries; tag-triggered crates.io publish

## [0.1.6] - 2026-05-07

### Changed

- Track `Cargo.lock` for reproducible builds
- Dependency bumps; fix docs.rs metadata
- CI: checkout/cache actions → v5 (Node 24)

## [0.1.5] - 2026-04-28

### Added

- Homebrew formula + release automation to bump the tap
- Optional NetCDF support; MSRV **Rust 1.95**
- Viewer: Zarr stores as first-class snapshot rows; richer metadata / kv_tables (NetCDF walk, column stats)

### Fixed

- Homebrew: declare `hdf5` for NetCDF builds
- Wide delimited rendering; clippy / CI lib setup

### Changed

- CLI: use clap’s built-in `--version`

## [0.1.4] - 2026-04-14

### Changed

- Dependency refresh

## [0.1.3] - 2026-04-13

### Changed

- Bump lofty / ZahirScan after lofty 0.23.3 yank

## [0.1.2] - 2026-04-08

### Added

- Config: `run_snapshot_on_startup`
- Directory enhance-policy header; duplicate-group label disambiguation
- Log viewer: head+tail for oversized log files

## [0.1.1] - 2026-04-02

### Fixed

- Viewer: theme on async workers; parallel Markdown tables; last-snapshot rename

## [0.1.0] - 2026-04-02

### Added

- Initial public release: Nefaxer-backed TUI catalog browser with ZahirScan enhance, Snapshot / Delta / Lenses, and right-pane previews

[0.3.0]: https://github.com/Latka-Industries/UBLX/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/Latka-Industries/UBLX/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/Latka-Industries/UBLX/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/Latka-Industries/UBLX/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/Latka-Industries/UBLX/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/Latka-Industries/UBLX/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Latka-Industries/UBLX/compare/v0.1.14...v0.2.0
[0.1.14]: https://github.com/Latka-Industries/UBLX/compare/v0.1.13...v0.1.14
[0.1.13]: https://github.com/Latka-Industries/UBLX/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/Latka-Industries/UBLX/compare/v0.1.11...v0.1.12
[0.1.11]: https://github.com/Latka-Industries/UBLX/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/Latka-Industries/UBLX/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/Latka-Industries/UBLX/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/Latka-Industries/UBLX/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/Latka-Industries/UBLX/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/Latka-Industries/UBLX/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/Latka-Industries/UBLX/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/Latka-Industries/UBLX/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/Latka-Industries/UBLX/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/Latka-Industries/UBLX/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/Latka-Industries/UBLX/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Latka-Industries/UBLX/releases/tag/v0.1.0
