# CLI (`src/cli`)

Headless catalog subcommands — no ratatui / TUI deps.

| Module      | Role                                                                 |
| ----------- | -------------------------------------------------------------------- |
| **catalog** | Resolve DIR → `.ublx` path; open read connection; snapshot-in-progress heuristic |
| **output**  | Shared JSON / string-list emit                                       |
| **query**   | `ublx query` — list / filter / detail / delta / lenses (THI-153)     |
| **doctor**  | `ublx doctor` — PASS/WARN/FAIL report, `--fix`, snapshot lock (THI-154) |

Clap definitions live in `src/cli_parser.rs` (`Commands`, `QueryCli`, `DoctorCli`). `main` dispatches via `cli::run` when a subcommand is present; otherwise the existing TUI / `-s`/`-f`/`-x` path runs.
