//! Lens (playlist) tables and operations: path, lens, `lens_path`.
//! One file can be in multiple lenses; path table normalizes path strings.

use std::path::Path;

use rusqlite::Connection;

use super::SnapshotTuiRow;
use super::consts::UblxDbStatements;
use super::core::open_for_snapshot_tui_read;

/// Load lens names in id order using an existing connection (e.g. shared with TUI cold start).
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` query errors.
pub fn load_lens_names_from_conn(conn: &Connection) -> Result<Vec<String>, anyhow::Error> {
    let mut stmt = conn.prepare(UblxDbStatements::SELECT_LENS_NAMES)?;
    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}

/// Load lens names in id order for the TUI left bar. Returns empty if DB missing or no lenses.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` open/query errors.
pub fn load_lens_names(db_path: &Path) -> Result<Vec<String>, anyhow::Error> {
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let conn = open_for_snapshot_tui_read(db_path)?;
    load_lens_names_from_conn(&conn)
}

/// Load (path, category, size) rows for a lens by name, ordered by position. Returns empty if lens not found or DB missing.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` open/query errors.
pub fn load_lens_paths(
    db_path: &Path,
    lens_name: &str,
) -> Result<Vec<SnapshotTuiRow>, anyhow::Error> {
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let conn = open_for_snapshot_tui_read(db_path)?;
    let lens_id: i64 = match conn.query_row(
        UblxDbStatements::SELECT_LENS_ID_BY_NAME,
        [lens_name],
        |row| row.get(0),
    ) {
        Ok(id) => id,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut stmt = conn.prepare(UblxDbStatements::SELECT_LENS_ROWS_FOR_TUI)?;
    let rows = stmt.query_map([lens_id], |row| {
        let size: i64 = row.get(2)?;
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            size.max(0).cast_unsigned(),
        ))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Create a lens by name. Returns the new lens id. Errors if name already exists.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors (e.g. unique constraint if the name exists).
pub fn create_lens(db_path: &Path, name: &str) -> Result<i64, anyhow::Error> {
    let conn = Connection::open(db_path)?;
    conn.execute(UblxDbStatements::INSERT_LENS, [name])?;
    Ok(conn.last_insert_rowid())
}

/// Ensure path exists in path table; return its id.
fn get_or_create_path_id(conn: &Connection, path: &str) -> Result<i64, anyhow::Error> {
    conn.execute(UblxDbStatements::INSERT_PATH, [path])?;
    let id: i64 = conn.query_row(UblxDbStatements::SELECT_PATH_ID_BY_PATH, [path], |row| {
        row.get(0)
    })?;
    Ok(id)
}

/// Add a path to a lens at the given position. Creates path in path table if needed. Replaces any existing (`lens_id`, `path_id`) with new position.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors (e.g. lens not found).
pub fn add_path_to_lens(
    db_path: &Path,
    lens_name: &str,
    path: &str,
    position: i64,
) -> Result<(), anyhow::Error> {
    let conn = Connection::open(db_path)?;
    let lens_id: i64 = conn.query_row(
        UblxDbStatements::SELECT_LENS_ID_BY_NAME,
        [lens_name],
        |row| row.get(0),
    )?;
    let path_id = get_or_create_path_id(&conn, path)?;
    conn.execute(
        UblxDbStatements::INSERT_LENS_PATH,
        [lens_id, path_id, position],
    )?;
    Ok(())
}

/// Remove a path from a lens. No-op if the path is not in the lens.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` open/execute errors.
pub fn remove_path_from_lens(
    db_path: &Path,
    lens_name: &str,
    path: &str,
) -> Result<(), anyhow::Error> {
    if !db_path.exists() {
        return Ok(());
    }
    let conn = Connection::open(db_path)?;
    conn.execute(UblxDbStatements::DELETE_LENS_PATH_ROW, (lens_name, path))?;
    Ok(())
}

/// Rename a lens. Errors if new name is empty or already exists.
///
/// # Errors
///
/// Returns [`anyhow::Error`] if the new name is empty, the old lens is missing, or `SQLite` reports a conflict.
pub fn rename_lens(db_path: &Path, old_name: &str, new_name: &str) -> Result<(), anyhow::Error> {
    if new_name.trim().is_empty() {
        return Err(anyhow::anyhow!("lens name cannot be empty"));
    }
    let conn = Connection::open(db_path)?;
    let changed = conn.execute(UblxDbStatements::UPDATE_LENS_NAME, (old_name, new_name))?;
    if changed == 0 {
        return Err(anyhow::anyhow!("lens not found: {old_name}"));
    }
    Ok(())
}

/// Delete a lens and all its path associations (`lens_path` rows removed by CASCADE).
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` open/execute errors.
pub fn delete_lens(db_path: &Path, lens_name: &str) -> Result<(), anyhow::Error> {
    if !db_path.exists() {
        return Ok(());
    }
    let conn = Connection::open(db_path)?;
    conn.execute(UblxDbStatements::DELETE_LENS, [lens_name])?;
    Ok(())
}

/// Update stored path strings when a file or folder is renamed on disk (lens membership).
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors.
pub fn rename_path_string(db_path: &Path, old: &str, new: &str) -> Result<(), anyhow::Error> {
    if !db_path.exists() {
        return Ok(());
    }
    let conn = Connection::open(db_path)?;
    conn.execute(UblxDbStatements::UPDATE_PATH_STRING, (new, old))?;
    Ok(())
}

/// Remove `path` row after the entry was deleted or moved from disk.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors.
pub fn delete_path_row(db_path: &Path, path: &str) -> Result<(), anyhow::Error> {
    if !db_path.exists() {
        return Ok(());
    }
    let conn = Connection::open(db_path)?;
    conn.execute(UblxDbStatements::DELETE_PATH_ROW, [path])?;
    Ok(())
}
