//! Shared read-only catalog queries for `ublx query` and `ublx serve`.

use std::fmt;

use rusqlite::{Connection, Row};
use serde::Serialize;

use crate::engine::db_ops::UblxDbStatements;

/// One snapshot (or lens) row for JSON / tables.
#[derive(Debug, Clone, Serialize)]
pub struct EntryRow {
    pub path: String,
    pub category: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zahir: Option<serde_json::Value>,
}

/// One `delta_log` row.
#[derive(Debug, Clone, Serialize)]
pub struct DeltaRow {
    pub created_ns: i64,
    pub path: String,
    pub delta_type: String,
}

/// Filters for listing snapshot entries.
#[derive(Debug, Clone, Default)]
pub struct EntryListFilter<'a> {
    pub category: Option<&'a str>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub contains: Option<&'a str>,
}

impl<'a> EntryListFilter<'a> {
    #[must_use]
    pub fn new(
        category: Option<&'a str>,
        min_size: Option<u64>,
        max_size: Option<u64>,
        contains: Option<&'a str>,
    ) -> Self {
        Self {
            category,
            min_size,
            max_size,
            contains,
        }
    }
}

/// Missing path / lens — map to HTTP 404 in serve; bail message for CLI.
#[derive(Debug)]
pub struct CatalogNotFound {
    pub kind: NotFoundKind,
    pub name: String,
}

#[derive(Debug, Clone, Copy)]
pub enum NotFoundKind {
    Path,
    Lens,
}

impl CatalogNotFound {
    #[must_use]
    pub fn path(name: impl Into<String>) -> Self {
        Self {
            kind: NotFoundKind::Path,
            name: name.into(),
        }
    }

    #[must_use]
    pub fn lens(name: impl Into<String>) -> Self {
        Self {
            kind: NotFoundKind::Lens,
            name: name.into(),
        }
    }
}

impl fmt::Display for CatalogNotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            NotFoundKind::Path => write!(f, "path not found in catalog: {}", self.name),
            NotFoundKind::Lens => write!(f, "lens not found: {}", self.name),
        }
    }
}

impl std::error::Error for CatalogNotFound {}

/// True when `err` (or a cause) is [`CatalogNotFound`].
#[must_use]
pub fn is_not_found(err: &anyhow::Error) -> bool {
    err.downcast_ref::<CatalogNotFound>().is_some()
        || err
            .chain()
            .any(|c| c.downcast_ref::<CatalogNotFound>().is_some())
}

/// List snapshot categories (distinct).
///
/// # Errors
///
/// Propagates `SQLite` failures.
pub fn list_categories(conn: &Connection) -> Result<Vec<String>, anyhow::Error> {
    query_strings(conn, UblxDbStatements::SELECT_SNAPSHOT_CATEGORIES)
}

/// List lens names.
///
/// # Errors
///
/// Propagates `SQLite` failures.
pub fn list_lens_names(conn: &Connection) -> Result<Vec<String>, anyhow::Error> {
    query_strings(conn, UblxDbStatements::SELECT_LENS_NAMES)
}

/// Paths in a named lens (ordered).
///
/// # Errors
///
/// Returns [`CatalogNotFound`] when the lens is missing, or `SQLite` failures.
pub fn list_lens_entries(conn: &Connection, lens: &str) -> Result<Vec<EntryRow>, anyhow::Error> {
    let mut stmt = conn.prepare(UblxDbStatements::SELECT_LENS_ID_BY_NAME)?;
    let lens_id: Option<i64> = stmt
        .query_row(rusqlite::params![lens], |row| row.get(0))
        .ok();
    let Some(lens_id) = lens_id else {
        return Err(CatalogNotFound::lens(lens).into());
    };
    let mut stmt = conn.prepare(UblxDbStatements::SELECT_LENS_ROWS_FOR_TUI)?;
    Ok(stmt
        .query_map(rusqlite::params![lens_id], entry_from_row)?
        .collect::<Result<Vec<_>, _>>()?)
}

/// Delta log rows (newest first), optional type filter (`added` / `mod` / `removed`;
/// aliases `modified`, `add`, `remove` accepted).
///
/// # Errors
///
/// Returns an error when the type is invalid or `SQLite` fails.
pub fn list_delta(
    conn: &Connection,
    delta_type: Option<&str>,
) -> Result<Vec<DeltaRow>, anyhow::Error> {
    let delta_type = delta_type.map(canonicalize_delta_type).transpose()?;
    let sql = if delta_type.is_some() {
        "SELECT created_ns, path, delta_type FROM delta_log WHERE delta_type = ?1 ORDER BY created_ns DESC, path"
    } else {
        "SELECT created_ns, path, delta_type FROM delta_log ORDER BY created_ns DESC, path"
    };
    let mut stmt = conn.prepare(sql)?;
    let map = |row: &Row<'_>| {
        Ok(DeltaRow {
            created_ns: row.get(0)?,
            path: row.get(1)?,
            delta_type: row.get(2)?,
        })
    };
    if let Some(t) = delta_type {
        Ok(stmt
            .query_map(rusqlite::params![t], map)?
            .collect::<Result<Vec<_>, _>>()?)
    } else {
        Ok(stmt.query_map([], map)?.collect::<Result<Vec<_>, _>>()?)
    }
}

/// One snapshot row by exact relative path; optional `zahir_json`.
///
/// # Errors
///
/// Returns [`CatalogNotFound`] when the path is missing, or `SQLite` failures.
pub fn entry_detail(
    conn: &Connection,
    path: &str,
    include_zahir: bool,
) -> Result<EntryRow, anyhow::Error> {
    let mut stmt =
        conn.prepare("SELECT path, COALESCE(category, ''), size FROM snapshot WHERE path = ?1")?;
    let mut row = stmt
        .query_row(rusqlite::params![path], entry_from_row)
        .map_err(|_| CatalogNotFound::path(path))?;

    if include_zahir {
        let mut zstmt = conn.prepare(UblxDbStatements::SELECT_SNAPSHOT_ZAHIR_JSON_BY_PATH)?;
        let zahir: Option<String> = zstmt
            .query_row(rusqlite::params![path], |r| r.get::<_, Option<String>>(0))
            .unwrap_or(None);
        row.zahir = parse_zahir_value(zahir.as_deref());
    }
    Ok(row)
}

/// List snapshot entries with optional filters (no zahir).
///
/// # Errors
///
/// Propagates `SQLite` failures.
pub fn list_entries(
    conn: &Connection,
    filter: &EntryListFilter<'_>,
) -> Result<Vec<EntryRow>, anyhow::Error> {
    let mut rows = load_entries(conn, filter.category)?;
    if let Some(min) = filter.min_size {
        rows.retain(|r| r.size >= min);
    }
    if let Some(max) = filter.max_size {
        rows.retain(|r| r.size <= max);
    }
    if let Some(needle) = filter.contains {
        rows.retain(|r| r.path.contains(needle));
    }
    Ok(rows)
}

fn load_entries(conn: &Connection, category: Option<&str>) -> Result<Vec<EntryRow>, anyhow::Error> {
    if let Some(cat) = category {
        let mut stmt = conn.prepare(UblxDbStatements::SELECT_SNAPSHOT_ROWS_FOR_TUI_BY_CATEGORY)?;
        Ok(stmt
            .query_map(rusqlite::params![cat], entry_from_row)?
            .collect::<Result<Vec<_>, _>>()?)
    } else {
        let mut stmt = conn.prepare(UblxDbStatements::SELECT_SNAPSHOT_ROWS_FOR_TUI_ALL)?;
        Ok(stmt
            .query_map([], entry_from_row)?
            .collect::<Result<Vec<_>, _>>()?)
    }
}

fn entry_from_row(row: &Row<'_>) -> rusqlite::Result<EntryRow> {
    let size: i64 = row.get(2)?;
    Ok(EntryRow {
        path: row.get(0)?,
        category: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        size: size.max(0).cast_unsigned(),
        zahir: None,
    })
}

fn query_strings(conn: &Connection, sql: &str) -> Result<Vec<String>, anyhow::Error> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn canonicalize_delta_type(t: &str) -> Result<&'static str, anyhow::Error> {
    match t.to_ascii_lowercase().as_str() {
        "added" | "add" => Ok("added"),
        "mod" | "modified" | "modify" => Ok("mod"),
        "removed" | "remove" | "rm" => Ok("removed"),
        other => anyhow::bail!("invalid delta type {other:?}; expected added|mod|removed"),
    }
}

/// Parse stored `zahir_json` text into a JSON value for nested pretty output.
/// Invalid JSON falls back to a string value so nothing is dropped.
fn parse_zahir_value(raw: Option<&str>) -> Option<serde_json::Value> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    Some(serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.to_string())))
}
