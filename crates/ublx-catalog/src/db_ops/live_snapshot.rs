//! Incremental updates to the live `snapshot` table (user rename/delete) without a full nefax index.

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use rusqlite::{Connection, Error as SqliteError};

use crate::paths::UblxPaths;
use crate::util::snapshot_rel_path_buf;
use crate::zahir::zahir_metadata_name_from_indexed_file;

use super::consts::{UblxDbCategory, UblxDbStatements};

fn mtime_ns_from_meta(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_nanos()).ok())
        .unwrap_or(0)
}

/// Remove one `snapshot` row after the path was deleted on disk.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors.
pub fn delete_snapshot_row(db_path: &Path, rel_path: &str) -> Result<(), anyhow::Error> {
    if !db_path.exists() {
        return Ok(());
    }
    let conn = Connection::open(db_path)?;
    conn.execute(UblxDbStatements::DELETE_SNAPSHOT_ROW, [rel_path])?;
    Ok(())
}

/// Repoint `snapshot` from `old_rel` to `new_rel` and refresh size/mtime/category from disk (`hash` set to NULL).
///
/// No-op if there is no row for `old_rel`.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on I/O or `SQLite` errors.
pub fn rename_snapshot_row(
    db_path: &Path,
    dir_to_ublx: &Path,
    ublx_paths: Option<&UblxPaths>,
    old_rel: &str,
    new_rel: &str,
) -> Result<(), anyhow::Error> {
    if !db_path.exists() {
        return Ok(());
    }
    let full_new = dir_to_ublx.join(snapshot_rel_path_buf(new_rel));
    let meta = fs::metadata(&full_new)?;
    let mtime_ns = mtime_ns_from_meta(&meta);
    let size_i = if meta.is_dir() {
        0_i64
    } else {
        i64::try_from(meta.len()).unwrap_or(i64::MAX)
    };
    let hint = zahir_metadata_name_from_indexed_file(&full_new, new_rel);
    let category = UblxDbCategory::get_category_for_path(&full_new, ublx_paths, hint.as_deref());

    let conn = Connection::open(db_path)?;
    let zahir_json: String = match conn.query_row(
        "SELECT COALESCE(zahir_json, '') FROM snapshot WHERE path = ?1",
        [old_rel],
        |row| row.get::<_, String>(0),
    ) {
        Ok(s) => s,
        Err(SqliteError::QueryReturnedNoRows) => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    conn.execute(
        UblxDbStatements::UPDATE_SNAPSHOT_RENAME_IN_PLACE,
        rusqlite::params![
            new_rel,
            mtime_ns,
            size_i,
            Option::<Vec<u8>>::None,
            category,
            zahir_json,
            old_rel,
        ],
    )?;
    Ok(())
}
