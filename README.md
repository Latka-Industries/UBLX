# UBLX

[![Crates.io](https://img.shields.io/crates/v/ublx.svg)](https://crates.io/crates/ublx)
[![docs.rs](https://img.shields.io/docsrs/ublx)](https://docs.rs/ublx)
![Build](https://github.com/Latka-Industries/UBLX/workflows/Build/badge.svg)
![UBLX Web](https://github.com/Latka-Industries/UBLX/workflows/UBLX%20Web/badge.svg)
![Rust](https://img.shields.io/badge/rust-1.95-orange.svg)

_[Ublx ... Safe when taken as directed.][ubik]_

**TUI that turns a directory into a flat, navigable catalog** — index once, enrich on demand, browse in the terminal. Indexing uses [Nefaxer][nefaxer]; deep metadata uses [ZahirScan][zahirscan] when you enhance. Headless `query` / `doctor` ship by default; optional `serve` exposes an HTTP API, and `ui` embeds a browser SPA.

**In active development — expect breaking changes.**

## Install

### Homebrew

```bash
brew tap Latka-Industries/ublx https://github.com/Latka-Industries/UBLX
brew install Latka-Industries/ublx/ublx
```

Homebrew builds with the embedded serve UI (`--features ui`).

### Cargo

```bash
cargo install ublx                     # TUI + query/doctor (default)
cargo install ublx --features serve    # + `ublx serve` HTTP API
cargo install ublx --features ui       # + serve + embedded Leptos SPA
cargo install ublx --no-default-features --features serve   # serve/query without TUI
```

Default crates.io install is TUI + catalog CLI. `serve` / `ui` are opt-in; `ui` implies `serve`. Catalog path resolve, SQLite ops, and headless open/read live in the internal `ublx-catalog` workspace crate — one published `ublx` binary.

## Quick start

```bash
ublx /path/to/your/project
```

Headless index: `ublx --snapshot-only /path/to/project` · Full metadata: `ublx --full-snapshot`

Catalog CLI (after an index exists):

```bash
ublx query . --categories
ublx query . --category Code --json
ublx query . --path src/main.rs --zahir
ublx query . --contains src --limit 20
ublx doctor .
ublx doctor --fix .    # remove leftover tmp/wal/shm (blocked while a snapshot is writing)
```

With a running serve (`--features serve` or `ui`), point query/doctor at it:

```bash
export UBLX_URL=http://127.0.0.1:8787
ublx query --contains src --json
ublx doctor --json
```

See `ublx --help`, `ublx query --help`, `ublx doctor --help`. With `--features serve` (or `ui`): `ublx serve --help`.

## Documentation

Full guides, config tables, TUI keys, and workflows live on the docs site (README here stays minimal).

|                               |                                                   |
| ----------------------------- | ------------------------------------------------- |
| **[Install][ublx-gs]**        | Homebrew, Cargo, prerequisites, first run         |
| [CLI][ublx-cli]               | `ublx --help`, headless flags, `query` / `doctor` |
| [Configuration][ublx-config]  | `ublx.toml`, enhance policies, themes             |
| [TUI & modes][ublx-tui]       | Snapshot, Delta, Lenses, panes, keybindings       |
| [Guides][ublx-guides]         | Path-only vs enhance, headless export, lenses     |
| [FAQ][ublx-faq]               | Common questions                                  |
| **[API (docs.rs)][ublx-api]** | Rust crate reference                              |

Not a file manager — for that, see [yazi][yazi]. UBLX targets **project trees**: fast catalogs, previews, diffs, and export.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).

[ubik]: https://bookshop.org/p/books/ubik-philip-k-dick/1fc432e3ade32290
[nefaxer]: https://github.com/Latka-Industries/nefaxer
[zahirscan]: https://github.com/Latka-Industries/zahirscan
[yazi]: https://github.com/sxyazi/yazi
[ublx-gs]: https://ublx.dev/getting-started
[ublx-cli]: https://ublx.dev/cli
[ublx-config]: https://ublx.dev/configuration
[ublx-tui]: https://ublx.dev/tui/
[ublx-guides]: https://ublx.dev/guides/
[ublx-faq]: https://ublx.dev/guides/faq
[ublx-api]: https://docs.rs/ublx
