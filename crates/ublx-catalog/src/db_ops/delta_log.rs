//! Mutations to `delta_log` after user-driven rename/delete (live `.ublx`), keeping Delta tab consistent.

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use rusqlite::Connection;

use crate::util::{get_created_ns, snapshot_rel_path_buf};

use super::consts::UblxDbStatements;

fn mtime_ns_from_meta(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_nanos()).ok())
        .unwrap_or(0)
}

/// Rewrite historical `delta_log` rows from `old_path` to `new_path`, then append one `removed` (old name)
/// and one `added` (new name) with the same `created_ns` — same idea as a delete + add in the nefax diff.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on I/O or `SQLite` errors.
pub fn record_delta_log_rename(
    db_path: &Path,
    root: &Path,
    old_path: &str,
    new_path: &str,
) -> Result<(), anyhow::Error> {
    if !db_path.exists() {
        return Ok(());
    }
    let abs_new = root.join(snapshot_rel_path_buf(new_path));
    let meta = fs::metadata(&abs_new)?;
    let mtime_ns = mtime_ns_from_meta(&meta);
    let size_i = if meta.is_dir() {
        0_i64
    } else {
        i64::try_from(meta.len()).unwrap_or(i64::MAX)
    };

    let mut conn = Connection::open(db_path)?;
    let tx = conn.transaction()?;
    tx.execute(
        UblxDbStatements::UPDATE_DELTA_LOG_PATH,
        (new_path, old_path),
    )?;
    let created_ns = get_created_ns();
    tx.execute(
        UblxDbStatements::INSERT_DELTA_LOG,
        rusqlite::params![
            created_ns,
            old_path,
            None::<i64>,
            None::<i64>,
            None::<Vec<u8>>,
            "removed",
        ],
    )?;
    tx.execute(
        UblxDbStatements::INSERT_DELTA_LOG,
        rusqlite::params![
            created_ns,
            new_path,
            mtime_ns,
            size_i,
            None::<Vec<u8>>,
            "added",
        ],
    )?;
    tx.commit()?;
    Ok(())
}

/// Append a single `removed` row for a path (user delete on disk). Does not erase prior history for that path.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors.
pub fn insert_delta_log_removed(db_path: &Path, path: &str) -> Result<(), anyhow::Error> {
    if !db_path.exists() {
        return Ok(());
    }
    let conn = Connection::open(db_path)?;
    let created_ns = get_created_ns();
    conn.execute(
        UblxDbStatements::INSERT_DELTA_LOG,
        rusqlite::params![
            created_ns,
            path,
            None::<i64>,
            None::<i64>,
            None::<Vec<u8>>,
            "removed",
        ],
    )?;
    Ok(())
}
