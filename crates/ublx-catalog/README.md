# ublx-catalog

Shared catalog layer for [UBLX](https://github.com/Latka-Industries/UBLX): path resolve (`UblxPaths`), SQLite schema / `db_ops`, and headless open/read helpers used by `ublx query`, `ublx doctor`, and `ublx serve`.

This crate is published so the main `ublx` binary can depend on it from crates.io. Most users should install **`ublx`**, not this crate directly.

## License

Dual-licensed under MIT or Apache-2.0 (same as UBLX).
