use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use log::{debug, warn};
use rayon::prelude::*;
use rusqlite::types::Value;
use rusqlite::{Connection, Statement, Transaction, params_from_iter};

use crate::nefax::{NefaxDiff, NefaxPathMeta, NefaxResult};
use crate::paths::UblxPaths;
use crate::util::snapshot_rel_path_buf;
use crate::zahir::{
    ZahirOutput, zahir_metadata_name_from_indexed_file, zahir_output_to_json_for_path,
};

use super::consts::{
    DeltaType, SNAPSHOT_INSERT_PREP_PARALLEL, UblxDbCategory, UblxDbSchema, UblxDbStatements,
};

/// How often to emit [`debug_snapshot_write_progress`] (also logs the first step, and the last when `total` is known).
pub const SNAPSHOT_DB_WRITE_PROGRESS_LOG_EVERY: u64 = 10_000;

/// Debug progress while building `.ublx_tmp` (snapshot inserts, streamed Zahir updates, etc.).
/// With `total`: logs at 1, every [`SNAPSHOT_DB_WRITE_PROGRESS_LOG_EVERY`], and at `total`.
/// Without `total` (unknown count): logs at 1 and every [`SNAPSHOT_DB_WRITE_PROGRESS_LOG_EVERY`] steps.
#[inline]
pub fn debug_snapshot_write_progress(phase: &str, n: u64, total: Option<u64>) {
    let every = SNAPSHOT_DB_WRITE_PROGRESS_LOG_EVERY;
    let log = match total {
        Some(0) => return,
        Some(t) => n == 1 || n == t || n.is_multiple_of(every),
        None => n == 1 || n.is_multiple_of(every),
    };
    if !log {
        return;
    }
    match total {
        Some(t) => debug!("snapshot DB: {n} of {t} — {phase}"),
        None => debug!("snapshot DB: {n} — {phase} (so far)"),
    }
}

/// Prior snapshot maps and the enhance-policy predicate passed through snapshot rebuild.
///
/// `batch_zahir_for_path` is the caller's index-time Zahir policy (in `ublx`:
/// `UblxOpts::batch_zahir_for_path`) so the catalog crate stays free of app options. It must be
/// [`Sync`]: row preparation runs under rayon for large snapshots.
pub struct SnapshotPriorContext<'a> {
    pub prior_zahir_json: &'a HashMap<String, String>,
    pub prior_category: &'a HashMap<String, String>,
    pub batch_zahir_for_path: &'a (dyn Fn(&str) -> bool + Sync),
}

/// Get the full path and the path string for a given path.
#[must_use]
pub fn get_full_path_and_path_str(dir_to_ublx: &Path, path_ref: &Path) -> (PathBuf, String) {
    let full_path = dir_to_ublx.join(path_ref);
    let path_str = path_ref.to_string_lossy().into_owned();
    (full_path, path_str)
}

/// Open or create the DB at `dir_to_ublx` and ensure all ublx tables exist. Returns the DB path.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` I/O or schema initialization errors.
pub fn ensure_ublx_and_db(dir_to_ublx: &Path) -> Result<PathBuf, anyhow::Error> {
    let paths = UblxPaths::new(dir_to_ublx);
    let _ = paths.ensure_db_dir()?;
    let path = paths.db();
    let conn = Connection::open(&path)?;
    conn.execute_batch(&UblxDbSchema::create_ublx_db_sql())?;
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_snapshot_category_path ON snapshot (category, path)",
        [],
    );
    Ok(path)
}

/// Category for a snapshot row: reuse prior DB category when present so Zahir enrichment does not move files between TUI categories; else path-type hint (same as first-time index without prior row).
fn category_for_snapshot_row(
    full_path: &Path,
    path_str: &str,
    ublx_paths: Option<&UblxPaths>,
    prior_category: &HashMap<String, String>,
) -> String {
    if let Some(c) = prior_category.get(path_str)
        && !c.is_empty()
    {
        return c.clone();
    }
    let hint = zahir_metadata_name_from_indexed_file(full_path, path_str);
    UblxDbCategory::get_category_for_path(full_path, ublx_paths, hint.as_deref())
}

#[must_use]
pub fn prepare_results_for_snapshot_insertion(
    dir_to_ublx: &Path,
    path_ref: &Path,
    ublx_paths: Option<&UblxPaths>,
    zahir_output_by_path: &HashMap<String, &ZahirOutput>,
    prior_zahir_json: &std::collections::HashMap<String, String>,
    prior_category: &HashMap<String, String>,
    batch_zahir_for_path: &(dyn Fn(&str) -> bool + Sync),
) -> (String, String, String) {
    let (full_path, path_str) = get_full_path_and_path_str(dir_to_ublx, path_ref);
    let category = category_for_snapshot_row(&full_path, &path_str, ublx_paths, prior_category);
    let skip_batch = !batch_zahir_for_path(&path_str);
    if skip_batch {
        let zahir_json = prior_zahir_json.get(&path_str).cloned().unwrap_or_default();
        return (path_str, category, zahir_json);
    }
    let zahir_output = zahir_output_by_path.get(&path_str);
    let zahir_json = zahir_output.map_or_else(
        || prior_zahir_json.get(&path_str).cloned().unwrap_or_default(),
        |o| zahir_output_to_json_for_path(Some(o), &full_path, &path_str),
    );
    (path_str, category, zahir_json)
}

/// Prepared row for snapshot insert: (`path_str`, category, `zahir_json`, `mtime_ns`, size, hash).
/// Hash is owned so the vec can be built in parallel (Send).
type SnapshotInsertRow = (String, String, String, i64, i64, Option<Vec<u8>>);

/// Rows per multi-value `INSERT` for sequential snapshot writes (6 bind params per row; stay under SQLite’s default 999 variable cap).
const SNAPSHOT_INSERT_BATCH_ROWS: usize = 150;

fn snapshot_batch_insert_sql(n_rows: usize) -> String {
    debug_assert!(n_rows > 0 && n_rows * 6 <= 999);
    let mut parts = Vec::with_capacity(n_rows);
    for i in 0..n_rows {
        let b = i * 6 + 1;
        parts.push(format!(
            "(?{b},?{},?{},?{},?{},?{})",
            b + 1,
            b + 2,
            b + 3,
            b + 4,
            b + 5
        ));
    }
    format!(
        "INSERT OR REPLACE INTO snapshot (path, mtime_ns, size, hash, category, zahir_json) VALUES {}",
        parts.join(",")
    )
}

fn execute_batched_snapshot_inserts(
    tx: &Transaction<'_>,
    mut prepared: Vec<SnapshotInsertRow>,
) -> Result<(), anyhow::Error> {
    let total = prepared.len() as u64;
    let mut done = 0u64;
    while !prepared.is_empty() {
        let n = prepared.len().min(SNAPSHOT_INSERT_BATCH_ROWS);
        let batch: Vec<SnapshotInsertRow> = prepared.drain(0..n).collect();
        let batch_len = batch.len();
        let sql = snapshot_batch_insert_sql(batch_len);
        let mut values: Vec<Value> = Vec::with_capacity(batch_len * 6);
        for (path_str, category, zahir_json, mtime_ns, size, hash_opt) in batch {
            values.push(Value::Text(path_str));
            values.push(Value::Integer(mtime_ns));
            values.push(Value::Integer(size));
            values.push(match hash_opt {
                None => Value::Null,
                Some(h) => Value::Blob(h),
            });
            values.push(Value::Text(category));
            values.push(Value::Text(zahir_json));
        }
        tx.execute(&sql, params_from_iter(values.iter()))?;
        done += batch_len as u64;
        debug_snapshot_write_progress("snapshot rows insert (Nefax)", done, Some(total));
    }
    Ok(())
}

/// Insert all nefax rows into the snapshot table (sequential snapshot path). Uses batched multi-row `INSERT`s inside the caller’s transaction.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` execute errors.
pub fn insert_results_into_snapshot(
    tx: &Transaction<'_>,
    nefax: &NefaxResult,
    dir_to_ublx: &Path,
    ublx_paths: Option<&UblxPaths>,
    zahir_output_by_path: &HashMap<String, &ZahirOutput>,
    prior: &SnapshotPriorContext<'_>,
) -> Result<(), anyhow::Error> {
    let prepared: Vec<SnapshotInsertRow> = if nefax.len() >= SNAPSHOT_INSERT_PREP_PARALLEL {
        let entries: Vec<_> = nefax.iter().collect();
        entries
            .par_iter()
            .map(|(path, meta)| {
                let (path_str, category, zahir_json) = prepare_results_for_snapshot_insertion(
                    dir_to_ublx,
                    path.as_path(),
                    ublx_paths,
                    zahir_output_by_path,
                    prior.prior_zahir_json,
                    prior.prior_category,
                    prior.batch_zahir_for_path,
                );
                let hash = meta.hash.as_ref().map(|h| h.as_slice().to_vec());
                (
                    path_str,
                    category,
                    zahir_json,
                    meta.mtime_ns,
                    meta.size.cast_signed(),
                    hash,
                )
            })
            .collect()
    } else {
        nefax
            .iter()
            .map(|(path, meta)| {
                let (path_str, category, zahir_json) = prepare_results_for_snapshot_insertion(
                    dir_to_ublx,
                    path,
                    ublx_paths,
                    zahir_output_by_path,
                    prior.prior_zahir_json,
                    prior.prior_category,
                    prior.batch_zahir_for_path,
                );
                let hash = meta.hash.as_ref().map(|h| h.as_slice().to_vec());
                (
                    path_str,
                    category,
                    zahir_json,
                    meta.mtime_ns,
                    meta.size.cast_signed(),
                    hash,
                )
            })
            .collect()
    };
    execute_batched_snapshot_inserts(tx, prepared)?;
    Ok(())
}

/// Insert all nefax rows (category from prior snapshot when present, else path hint; `zahir_json` from prior). For streaming: `zahir_json` updates applied later for paths that were sent.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` execute errors or filesystem metadata errors when inserting global config.
pub fn insert_nefax_only_into_snapshot(
    stmt: &mut Statement,
    nefax: &NefaxResult,
    dir_to_ublx: &Path,
    ublx_paths: Option<&UblxPaths>,
    prior: &SnapshotPriorContext<'_>,
) -> Result<(), anyhow::Error> {
    let total = nefax.len() as u64;
    for (i, (path, meta)) in nefax.iter().enumerate() {
        let (full_path, path_str) = get_full_path_and_path_str(dir_to_ublx, path);
        let category =
            category_for_snapshot_row(&full_path, &path_str, ublx_paths, prior.prior_category);
        let zahir_json = prior
            .prior_zahir_json
            .get(&path_str)
            .cloned()
            .unwrap_or_default();
        stmt.execute(rusqlite::params![
            path_str,
            meta.mtime_ns,
            meta.size.cast_signed(),
            meta.hash.as_ref().map(<[u8; 32]>::as_slice),
            category,
            zahir_json,
        ])?;
        debug_snapshot_write_progress("snapshot rows insert (Nefax)", i as u64 + 1, Some(total));
    }
    Ok(())
}

/// Insert `delta_log` rows for one delta type (added/modified/removed).
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` execute errors.
pub fn insert_results_into_delta_log_by_type(
    stmt: &mut Statement,
    nefax: &NefaxResult,
    diff: &NefaxDiff,
    delta_type: DeltaType,
    created_ns: i64,
) -> Result<(), anyhow::Error> {
    let diff_to_use = match delta_type {
        DeltaType::Added => &diff.added,
        DeltaType::Mod => &diff.modified,
        DeltaType::Removed => &diff.removed,
    };
    for path in diff_to_use {
        let path_str = path.to_string_lossy().into_owned();
        let (mtime_ns, size, hash) = match delta_type {
            DeltaType::Added | DeltaType::Mod => {
                // Paths in diff.added/diff.modified should match the current nefax run; if not, skip.
                let Some(meta) = nefax.get(path) else {
                    warn!(
                        "nefax missing metadata for {} path {}; skipping delta_log row",
                        delta_type.as_str(),
                        path_str
                    );
                    continue;
                };
                (
                    Some(meta.mtime_ns),
                    Some(meta.size.cast_signed()),
                    meta.hash.as_ref().map(<[u8; 32]>::as_slice),
                )
            }
            DeltaType::Removed => (None::<i64>, None::<i64>, None::<&[u8]>),
        };
        stmt.execute(rusqlite::params![
            created_ns,
            path_str,
            mtime_ns,
            size,
            hash,
            delta_type.as_str(),
        ])?;
    }
    Ok(())
}

pub struct NefaxFromGivenDB {
    db_path_to_use: PathBuf,
    table_name: String,
}

impl NefaxFromGivenDB {
    #[must_use]
    pub fn new(_dir_to_ublx: &Path, db_path: &Path) -> Self {
        // Always load prior from `.ublx` `snapshot`. A stale `.nefaxer` beside an empty snapshot
        // (first-run prompt → deferred index) produced a full prior from `paths`, empty nefax diff,
        // and (0,0,0) while the DB never filled — see `orchestrator::run` nefaxer delete when snapshot empty.
        Self {
            db_path_to_use: db_path.to_path_buf(),
            table_name: "snapshot".to_string(),
        }
    }

    /// Load the ublx snapshot table into a Nefax-compatible map. Returns `None` if the table is empty.
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] on `SQLite` open/query errors.
    pub fn load_nefax_from_given_db(self) -> Result<Option<NefaxResult>, anyhow::Error> {
        let conn = Connection::open(&self.db_path_to_use)?;
        let query = UblxDbStatements::create_query_for_nefax_from_db(&self.table_name);

        let mut stmt = conn.prepare(&query)?;

        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let mtime_ns: i64 = row.get(1)?;
            let size: i64 = row.get(2)?;
            let hash_blob: Option<Vec<u8>> = row.get(3)?;
            Ok((path, mtime_ns, size, hash_blob))
        })?;
        let nefax = Self::rows_to_nefax(rows)?;
        if nefax.is_empty() {
            Ok(None)
        } else {
            Ok(Some(nefax))
        }
    }

    /// Convert a list of rows to a Nefax-compatible map.
    fn rows_to_nefax(
        rows: impl Iterator<Item = rusqlite::Result<(String, i64, i64, Option<Vec<u8>>)>>,
    ) -> Result<NefaxResult, anyhow::Error> {
        let mut nefax = NefaxResult::new();
        for row in rows {
            let (path_str, mtime_ns, size, hash_blob) = row?;
            nefax_insert_snapshot_row(&mut nefax, &path_str, mtime_ns, size, hash_blob);
        }
        Ok(nefax)
    }
}

/// One snapshot row’s metadata into prior Nefax (skips absolute paths; same rules as [`NefaxFromGivenDB::load_nefax_from_given_db`]).
pub fn nefax_insert_snapshot_row(
    nefax: &mut NefaxResult,
    path_str: &str,
    mtime_ns: i64,
    size: i64,
    hash_blob: Option<Vec<u8>>,
) {
    // Drop synthetic global-config row (`insert_global_config_row_if_exists`): absolute path, not a nefax-relative key.
    // Local `ublx.toml` / `ublx.log` use the same display categories but stay relative — they must stay in prior nefax.
    if Path::new(path_str.trim()).is_absolute() {
        return;
    }
    let path = snapshot_rel_path_buf(path_str);
    let size_u = size.try_into().unwrap_or(0);
    let hash = hash_blob.filter(|b| b.len() == 32).map(|b| {
        let mut a = [0u8; 32];
        a.copy_from_slice(b.as_slice());
        a
    });
    nefax.insert(
        path,
        NefaxPathMeta {
            mtime_ns,
            size: size_u,
            hash,
        },
    );
}

/// True if `snapshot` has at least one row (schema present and populated).
#[must_use]
pub fn snapshot_table_has_rows(db_path: &Path) -> bool {
    let Ok(conn) = Connection::open(db_path) else {
        return false;
    };
    conn.query_row("SELECT 1 FROM snapshot LIMIT 1", [], |_r| Ok(()))
        .is_ok()
}

pub struct UblxCleanup {
    dir_to_ublx_abs: PathBuf,
}

impl UblxCleanup {
    #[must_use]
    pub fn new(dir_to_ublx: &Path) -> Self {
        Self {
            dir_to_ublx_abs: dir_to_ublx.to_path_buf(),
        }
    }

    /// Remove the nefaxer index file (`NEFAX_DB`) under `dir_to_ublx` if it exists. Call after the operation is complete (e.g. after writing ublx snapshot).
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when removing the file fails.
    pub fn delete_nefaxer_files(dir_to_ublx: &Path) -> Result<(), anyhow::Error> {
        let path = UblxPaths::new(dir_to_ublx).nefax_db();
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Remove temporary ublx files under `dir_to_ublx` if they exist.
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when removing auxiliary files fails.
    pub fn delete_ublx_tmp_files(dir_to_ublx: &Path) -> Result<(), anyhow::Error> {
        UblxPaths::new(dir_to_ublx).remove_aux_files()?;
        Ok(())
    }

    /// Remove the nefaxer index file and ublx tmp files under this cleanup's `dir_to_ublx_abs`. Call after the operation is complete (e.g. after writing ublx snapshot).
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when deleting nefaxer or tmp files fails.
    pub fn post_run_cleanup(&self) -> Result<(), anyhow::Error> {
        Self::delete_nefaxer_files(&self.dir_to_ublx_abs)?;
        Self::delete_ublx_tmp_files(&self.dir_to_ublx_abs)?;
        Ok(())
    }
}
