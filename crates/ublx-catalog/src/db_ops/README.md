# DB operations

SQLite `.ublx` database: schema, read/write, and higher-level ops (snapshot, delta, lenses, duplicates).

`category` includes zahir type strings (e.g. **Zarr** for a `.zarr` store root); see `UblxDbCategory` in **consts**.

## Tables

| Table         | Purpose                                                                                                       |
| ------------- | ------------------------------------------------------------------------------------------------------------- |
| **snapshot**  | One row per path: path, mtime_ns, size, hash, category, zahir_json. Primary index.                            |
| **settings**  | Cached disk/tuning (num_threads, drive_type, parallel_walk, config_source) to skip disk check when DB exists. |
| **delta_log** | Per-run changes: created_ns, path, mtime_ns, size, hash, delta_type (added/mod/removed).                      |
| **lens**      | Lenses (id, name).                                                                                            |
| **lens_path** | Paths in each lens (lens_id, path).                                                                           |

Schema and DDL live in **consts** (`UblxDbSchema`, `create_ublx_db_sql()`).

## Modules

| Module                 | Purpose                                                                                                        |
| ---------------------- | -------------------------------------------------------------------------------------------------------------- |
| **consts**             | Schema, table/column names, SQL strings (create, insert, select). `DeltaType`, `UblxDbCategory`.               |
| **core**               | Open/create DB, write snapshot/settings/delta_log, load snapshot rows. `ensure_ublx_and_db`, `SnapshotTuiRow`. |
| **delta_log**          | Delta log I/O.                                                                                                 |
| **live_snapshot**      | TUI “live” snapshot row reads while indexing.                                                                  |
| **path_resolver**      | Which file to read (`.ublx` vs `.ublx_tmp` during snapshot). `SnapshotReaderPreference`.                       |
| **utils**              | Delta diff (copy previous delta, write rows), `get_category_for_path`, cleanup.                                |
| **extract_duplicates** | Duplicate groups for the TUI (hash / size+blake3).                                                             |
| **lens_storage**       | Lens CRUD, paths in a lens, merge after snapshot.                                                              |
| **lens_export**        | Lens → Markdown export.                                                                                        |
| **zahir_export**       | Headless Zahir JSON export helpers.                                                                            |
