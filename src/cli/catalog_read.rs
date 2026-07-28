//! Shared read-only catalog queries for `ublx query` and `ublx serve`.

use std::fmt;

use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

use crate::engine::db_ops::UblxDbStatements;
#[cfg(feature = "serve")]
use crate::engine::db_ops::{DuplicateGroupingMode, load_duplicate_groups};

/// One snapshot (or lens) row for JSON / tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryRow {
    pub path: String,
    pub category: String,
    pub size: u64,
    /// Snapshot `mtime_ns` when loaded for detail (viewer footer). Omitted on list rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime_ns: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zahir: Option<serde_json::Value>,
}

/// One `delta_log` row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaRow {
    pub created_ns: i64,
    pub path: String,
    pub delta_type: String,
}

/// One duplicate group for JSON (`GET /duplicates`).
#[cfg(feature = "serve")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroupRow {
    /// Stable index within this response (for clients that select by id).
    pub id: usize,
    /// Left-pane label (shortest path in the group — same as TUI).
    pub label: String,
    pub paths: Vec<String>,
}

/// Duplicate listing payload.
#[cfg(feature = "serve")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatesResponse {
    /// `hash` or `name_size` (matches TUI tab suffix H / N/S).
    pub mode: String,
    pub groups: Vec<DuplicateGroupRow>,
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

/// Window into a filtered entry list (`GET /entries?limit=&offset=`).
#[derive(Debug, Clone, Copy)]
pub struct EntryListWindow {
    pub offset: usize,
    pub limit: usize,
}

/// Hard cap when callers pass `limit` (serve / query). Prevents accidental huge pages.
pub const ENTRY_LIST_LIMIT_MAX: usize = 10_000;

impl EntryListWindow {
    /// Clamp `limit` to `1..=ENTRY_LIST_LIMIT_MAX`.
    #[must_use]
    pub fn clamped(offset: usize, limit: usize) -> Self {
        Self {
            offset,
            limit: limit.clamp(1, ENTRY_LIST_LIMIT_MAX),
        }
    }
}

/// Windowed list payload (THI-205). Used when `limit` is set on `GET /entries`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryListPage {
    /// Rows matching filters (before limit/offset).
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub entries: Vec<EntryRow>,
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
#[cfg(feature = "serve")]
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

/// Duplicate groups for the catalog (read-only: no on-demand blake3 fill).
///
/// Uses stored hashes when present; otherwise `(basename, size)` grouping — same fallback as TUI
/// when `hash` is off / hashes are missing.
///
/// # Errors
///
/// Propagates `SQLite` / I/O failures from [`load_duplicate_groups`].
#[cfg(feature = "serve")]
pub fn list_duplicates(
    db_path: &std::path::Path,
    dir_to_ublx: &std::path::Path,
) -> Result<DuplicatesResponse, anyhow::Error> {
    let (groups, mode) = load_duplicate_groups(db_path, dir_to_ublx, false)?;
    let mode = match mode {
        DuplicateGroupingMode::Hash => "hash",
        DuplicateGroupingMode::NameSize => "name_size",
    };
    let groups = groups
        .into_iter()
        .enumerate()
        .map(|(id, g)| DuplicateGroupRow {
            id,
            label: g.representative_name().to_string(),
            paths: g.paths,
        })
        .collect();
    Ok(DuplicatesResponse {
        mode: mode.to_string(),
        groups,
    })
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

    let mut mstmt = conn.prepare(UblxDbStatements::SELECT_SNAPSHOT_MTIME_BY_PATH)?;
    row.mtime_ns = mstmt
        .query_row(rusqlite::params![path], |r| r.get::<_, i64>(0))
        .ok();

    if include_zahir {
        let mut zstmt = conn.prepare(UblxDbStatements::SELECT_SNAPSHOT_ZAHIR_JSON_BY_PATH)?;
        let zahir: Option<String> = zstmt
            .query_row(rusqlite::params![path], |r| r.get::<_, Option<String>>(0))
            .unwrap_or(None);
        row.zahir = parse_zahir_value(zahir.as_deref());
    }
    Ok(row)
}

/// List snapshot entries with optional filters (no zahir). Full result set.
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

/// Filtered entry page with SQL `LIMIT`/`OFFSET` (THI-205). Prefer this for large catalogs.
///
/// Order matches TUI list queries: `category, path` when unfiltered by category; `path` when
/// `category` is set.
///
/// # Errors
///
/// Propagates `SQLite` failures.
pub fn list_entries_page(
    conn: &Connection,
    filter: &EntryListFilter<'_>,
    window: EntryListWindow,
) -> Result<EntryListPage, anyhow::Error> {
    let window = EntryListWindow::clamped(window.offset, window.limit);
    let (where_sql, bind) = snapshot_list_where(filter);
    let order = if filter.category.is_some() {
        "ORDER BY path"
    } else {
        "ORDER BY category, path"
    };

    let count_sql = format!("SELECT COUNT(*) FROM snapshot {where_sql}");
    let total: i64 = {
        let mut stmt = conn.prepare(&count_sql)?;
        stmt.query_row(
            rusqlite::params_from_iter(bind.iter().map(value_from_bind)),
            |r| r.get(0),
        )?
    };
    let total = usize::try_from(total.max(0)).unwrap_or(usize::MAX);

    let select_sql =
        format!("SELECT path, category, size FROM snapshot {where_sql} {order} LIMIT ? OFFSET ?");
    let mut stmt = conn.prepare(&select_sql)?;
    let limit_i = i64::try_from(window.limit).unwrap_or(i64::MAX);
    let offset_i = i64::try_from(window.offset).unwrap_or(i64::MAX);
    let mut params: Vec<rusqlite::types::Value> = bind.iter().map(value_from_bind).collect();
    params.push(rusqlite::types::Value::Integer(limit_i));
    params.push(rusqlite::types::Value::Integer(offset_i));
    let entries = stmt
        .query_map(rusqlite::params_from_iter(params), entry_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(EntryListPage {
        total,
        offset: window.offset,
        limit: window.limit,
        entries,
    })
}

/// Bind values for dynamic snapshot list WHERE (owned for COUNT + SELECT).
enum ListBind {
    Text(String),
    Integer(i64),
}

fn value_from_bind(b: &ListBind) -> rusqlite::types::Value {
    match b {
        ListBind::Text(s) => rusqlite::types::Value::Text(s.clone()),
        ListBind::Integer(n) => rusqlite::types::Value::Integer(*n),
    }
}

fn snapshot_list_where(filter: &EntryListFilter<'_>) -> (String, Vec<ListBind>) {
    let mut clauses = Vec::new();
    let mut bind = Vec::new();
    if let Some(cat) = filter.category {
        clauses.push("category = ?");
        bind.push(ListBind::Text(cat.to_string()));
    }
    if let Some(needle) = filter.contains {
        clauses.push("path LIKE ? ESCAPE '\\'");
        bind.push(ListBind::Text(like_contains_pattern(needle)));
    }
    if let Some(min) = filter.min_size {
        clauses.push("size >= ?");
        bind.push(ListBind::Integer(i64::try_from(min).unwrap_or(i64::MAX)));
    }
    if let Some(max) = filter.max_size {
        clauses.push("size <= ?");
        bind.push(ListBind::Integer(i64::try_from(max).unwrap_or(i64::MAX)));
    }
    if clauses.is_empty() {
        (String::new(), bind)
    } else {
        (format!("WHERE {}", clauses.join(" AND ")), bind)
    }
}

/// `LIKE` pattern for substring match; escapes `\`, `%`, `_`.
fn like_contains_pattern(needle: &str) -> String {
    let mut out = String::from("%");
    for c in needle.chars() {
        match c {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('%');
    out
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
        mtime_ns: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn mem_snapshot() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE snapshot (
                path TEXT PRIMARY KEY,
                category TEXT,
                size INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO snapshot (path, category, size) VALUES
                ('a.rs', 'Code', 10),
                ('b.rs', 'Code', 20),
                ('c.md', 'Markdown', 5),
                ('dir/x_%y.rs', 'Code', 30),
                ('z.txt', 'Text', 100);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn list_entries_page_window_and_total() {
        let conn = mem_snapshot();
        let page = list_entries_page(
            &conn,
            &EntryListFilter::default(),
            EntryListWindow {
                offset: 0,
                limit: 2,
            },
        )
        .unwrap();
        assert_eq!(page.total, 5);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.limit, 2);
        assert_eq!(page.offset, 0);
    }

    #[test]
    fn list_entries_page_offset_past_end() {
        let conn = mem_snapshot();
        let page = list_entries_page(
            &conn,
            &EntryListFilter::default(),
            EntryListWindow {
                offset: 100,
                limit: 10,
            },
        )
        .unwrap();
        assert_eq!(page.total, 5);
        assert!(page.entries.is_empty());
    }

    #[test]
    fn list_entries_page_category_and_contains() {
        let conn = mem_snapshot();
        let page = list_entries_page(
            &conn,
            &EntryListFilter::new(Some("Code"), None, None, Some("b.")),
            EntryListWindow {
                offset: 0,
                limit: 50,
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.entries[0].path, "b.rs");
    }

    #[test]
    fn list_entries_page_like_escape() {
        let conn = mem_snapshot();
        let page = list_entries_page(
            &conn,
            &EntryListFilter::new(None, None, None, Some("x_%y")),
            EntryListWindow {
                offset: 0,
                limit: 10,
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.entries[0].path, "dir/x_%y.rs");
    }

    #[test]
    fn list_entries_page_clamps_limit() {
        let conn = mem_snapshot();
        let page = list_entries_page(
            &conn,
            &EntryListFilter::default(),
            EntryListWindow {
                offset: 0,
                limit: 0,
            },
        )
        .unwrap();
        assert_eq!(page.limit, 1);
        assert_eq!(page.entries.len(), 1);
    }
}
