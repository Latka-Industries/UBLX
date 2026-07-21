# CLI (`src/cli`)

Headless catalog subcommands — no ratatui / TUI deps.

| Module           | Role                                                                             |
| ---------------- | -------------------------------------------------------------------------------- |
| **catalog**      | Resolve DIR → `.ublx` path; open read connection; snapshot-in-progress heuristic |
| **catalog_read** | Shared list/detail/delta/lens queries (query + serve)                            |
| **output**       | Shared JSON / string-list emit                                                   |
| **query**        | `ublx query` — list / filter / detail / delta / lenses (THI-153)                 |
| **doctor**       | `ublx doctor` — PASS/WARN/FAIL report, `--fix`, snapshot lock (THI-154)          |
| **serve**        | `ublx serve` — local HTTP API via panza (THI-156)                                |

Clap definitions live in `src/cli_parser.rs` (`Commands`, `QueryCli`, `DoctorCli`, `ServeCli`). `main` dispatches via `cli::run` when a subcommand is present; otherwise the existing TUI / `-s`/`-f`/`-x` path runs.

## `ublx serve` (v0.1.13)

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
curl -s http://127.0.0.1:8787/lenses
```

Notes:

- `GET /roots` — indexed projects (same source as TUI switch); `PUT /roots/current` swaps the live catalog (blocked with 409 while a snapshot is `running`)
- `GET /doctor` — same diagnose report as `ublx doctor --json` for the current root (no `--fix` over HTTP)
- `POST /snapshot` — **202** + background job (TUI pipeline); `GET /snapshot` for `idle|running|done|failed`; catalog connection is reopened after rename
- `GET /categories` — exact category strings for `?category=` (case-sensitive, e.g. `Code` not `code`)
- `GET /delta?type=` — wire values `added` | `mod` | `removed` (`modified` accepted as alias for `mod`)

Hard nefax failures in the orchestrator can still process-exit (same as TUI on-demand snapshot). Prefer panza’s `GET /health` for liveness only.

Bind/health/static shell comes from [panza](https://crates.io/crates/panza) (`--host` / `--port` / `--open`). No embedded UI yet (`StaticMount::None`).
