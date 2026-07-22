# CLI (`src/cli`)

Headless catalog subcommands — no ratatui / TUI deps.

| Module           | Role                                                                             |
| ---------------- | -------------------------------------------------------------------------------- |
| **catalog**      | Resolve DIR → `.ublx` path; open read connection; snapshot-in-progress heuristic |
| **catalog_read** | Shared list/detail/delta/lens queries (query + serve)                            |
| **output**       | Shared JSON / string-list emit                                                   |
| **remote**       | HTTP client for `--url` / `UBLX_URL` (query + doctor → serve)                    |
| **query**        | `ublx query` — list / filter / detail / delta / lenses (THI-153)                 |
| **doctor**       | `ublx doctor` — PASS/WARN/FAIL report, `--fix`, snapshot lock (THI-154)          |
| **serve**        | `ublx serve` — local HTTP API via panza (THI-156)                                |

Clap definitions live in `src/cli_parser.rs` (`Commands`, `QueryCli`, `DoctorCli`, `ServeCli`). `main` dispatches via `cli::run` when a subcommand is present; otherwise the existing TUI / `-s`/`-f`/`-x` path runs.

## Remote client (v0.1.14)

`ublx query` and `ublx doctor` accept `--url <base>` (or env `UBLX_URL`). When set, `DIR` is ignored and the CLI talks to a running `ublx serve` over HTTP (same flags / table+JSON output).

```bash
export UBLX_URL=http://127.0.0.1:8787
ublx query --contains src --json
ublx query --path README.md --zahir
ublx query --delta --delta-type mod
ublx doctor --json
# --fix / --force are local-only with doctor
```

## `ublx serve` (v0.1.13+)

```bash
ublx serve . --port 8787
curl -s http://127.0.0.1:8787/health
curl -s http://127.0.0.1:8787/roots
curl -s http://127.0.0.1:8787/roots/current
curl -s -X PUT http://127.0.0.1:8787/roots/current \
  -H 'content-type: application/json' \
  -d '{"dir":"/path/to/other/indexed/project"}'
curl -s http://127.0.0.1:8787/doctor
curl -s -X POST http://127.0.0.1:8787/snapshot \
  -H 'content-type: application/json' \
  -d '{"enhance_all":false}'
curl -s http://127.0.0.1:8787/snapshot   # poll until state != running
curl -s http://127.0.0.1:8787/categories
curl -s 'http://127.0.0.1:8787/entries?category=Code&contains=src'
curl -s 'http://127.0.0.1:8787/entries/README.md?zahir=1'
curl -s 'http://127.0.0.1:8787/delta?type=mod'
curl -s http://127.0.0.1:8787/duplicates
curl -s http://127.0.0.1:8787/lenses
curl -s http://127.0.0.1:8787/settings/local
curl -s -X PATCH http://127.0.0.1:8787/settings/local \
  -H 'content-type: application/json' \
  -d '{"show_hidden_files":true}'
```
Notes:

- `GET /roots` — indexed projects (same source as TUI switch); `PUT /roots/current` swaps the live catalog (blocked with 409 while a snapshot is `running`)
- `GET /doctor` — same diagnose report as `ublx doctor --json` for the current root (no `--fix` over HTTP)
- `POST /snapshot` — **202** + background job (TUI pipeline); `GET /snapshot` for `idle|running|done|failed`; catalog connection is reopened after rename
- `GET /categories` — exact category strings for `?category=` (case-sensitive, e.g. `Code` not `code`)
- `GET /delta?type=` — wire values `added` | `mod` | `removed` (`modified` accepted as alias for `mod`)
- `GET /duplicates` — `{ mode: "hash"|"name_size", groups: [{ id, label, paths }] }` (read-only; no on-demand blake3 fill)
- `GET /entries/{*path}?zahir=1` — entry detail; when zahir is set, also `metadata_tables` / `writing_tables` (host-parsed KV / column-stat sections; honors effective `typed_column_tables`)
- `GET /content/{*path}` — disk file body for Viewer (`file_content_for_viewer`); `?format=text|html` (default: HTML when category is Markdown, else text). Path must stay under the current root.
- `GET /settings/{scope}` — `scope` is `global`|`local`; returns path, exists, live `toml` text, bools, layout, theme list, `bg_opacity`, `typed_column_tables` (`none`|`abbrev`|`full`), and `css` (effective palette → HSL tokens)
- `PATCH /settings/{scope}` — structured JSON fields only (no raw TOML body); response is the refreshed view (includes updated `css` / `typed_column_tables`)

Hard nefax failures in the orchestrator can still process-exit (same as TUI on-demand snapshot). Prefer panza’s `GET /health` for liveness only.

Bind/health/static shell comes from [panza](https://crates.io/crates/panza) (`--host` / `--port` / `--open`). No embedded UI yet (`StaticMount::None`).

## Planned: embedded web UI (v0.2.0 / THI-157)

Opt-in Cargo feature **`ui`**: Leptos CSR + **leptos-shadcn-ui**, embedded via `StaticMount::Embedded`, same-origin against the JSON API above. Default builds stay API-only.

Design / packaging: [`docs/WEB_UI.md`](../../docs/WEB_UI.md).
