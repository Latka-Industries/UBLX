//! `ublx query` — headless catalog query (THI-153).

use rusqlite::{Connection, Row};

use crate::cli::catalog::open_catalog_for_read;
use crate::cli::output::{emit_json, emit_string_list};
use crate::cli_parser::QueryCli;
use crate::engine::db_ops::{DeltaType, UblxDbStatements};
use crate::utils::format_bytes;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct EntryRow {
    path: String,
    category: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    zahir: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct DeltaRow {
    created_ns: i64,
    path: String,
    delta_type: String,
}

/// Run `ublx query` against the catalog for `DIR`.
///
/// # Errors
///
/// Returns `Err` when the directory/catalog cannot be opened or a query fails.
pub fn run(args: &QueryCli) -> Result<(), anyhow::Error> {
    let handle = open_catalog_for_read(&args.dir)?;
    let conn = &handle.conn;

    if args.categories {
        let cats = query_strings(conn, UblxDbStatements::SELECT_SNAPSHOT_CATEGORIES)?;
        return emit_string_list(&cats, "categories", args.json);
    }
    if args.lenses {
        let names = query_strings(conn, UblxDbStatements::SELECT_LENS_NAMES)?;
        return emit_string_list(&names, "lenses", args.json);
    }
    if let Some(ref name) = args.lens {
        return print_lens_paths(conn, name, args.json);
    }
    if args.delta {
        return print_delta(conn, args.delta_type.as_deref(), args.json);
    }
    if let Some(ref path) = args.path {
        return print_path_detail(conn, path, args.zahir, args.json);
    }

    print_entries(conn, args)
}

fn print_lens_paths(conn: &Connection, lens: &str, json: bool) -> Result<(), anyhow::Error> {
    let mut stmt = conn.prepare(UblxDbStatements::SELECT_LENS_ID_BY_NAME)?;
    let lens_id: Option<i64> = stmt
        .query_row(rusqlite::params![lens], |row| row.get(0))
        .ok();
    let Some(lens_id) = lens_id else {
        anyhow::bail!("lens not found: {lens}");
    };
    let mut stmt = conn.prepare(
        "SELECT p.path, COALESCE(s.category, ''), COALESCE(s.size, 0)
         FROM lens_path lp
         JOIN path p ON p.id = lp.path_id
         LEFT JOIN snapshot s ON s.path = p.path
         WHERE lp.lens_id = ?1
         ORDER BY lp.position",
    )?;
    let rows: Vec<EntryRow> = stmt
        .query_map(rusqlite::params![lens_id], entry_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    emit_entries(&rows, json)
}

fn print_delta(
    conn: &Connection,
    delta_type: Option<&str>,
    json: bool,
) -> Result<(), anyhow::Error> {
    if let Some(t) = delta_type {
        validate_delta_type(t)?;
    }
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
    let rows: Vec<DeltaRow> = if let Some(t) = delta_type {
        stmt.query_map(rusqlite::params![t], map)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([], map)?.collect::<Result<Vec<_>, _>>()?
    };
    if json {
        emit_json(&rows)?;
    } else {
        println!("{:<20} {:<8} PATH", "CREATED_NS", "TYPE");
        for r in &rows {
            println!("{:<20} {:<8} {}", r.created_ns, r.delta_type, r.path);
        }
        eprintln!("{} delta rows", rows.len());
    }
    Ok(())
}

fn print_path_detail(
    conn: &Connection,
    path: &str,
    include_zahir: bool,
    json: bool,
) -> Result<(), anyhow::Error> {
    let mut stmt =
        conn.prepare("SELECT path, COALESCE(category, ''), size FROM snapshot WHERE path = ?1")?;
    let mut row = stmt
        .query_row(rusqlite::params![path], entry_from_row)
        .map_err(|_| anyhow::anyhow!("path not found in catalog: {path}"))?;

    if include_zahir {
        let mut zstmt = conn.prepare(UblxDbStatements::SELECT_SNAPSHOT_ZAHIR_JSON_BY_PATH)?;
        let zahir: Option<String> = zstmt
            .query_row(rusqlite::params![path], |r| r.get::<_, Option<String>>(0))
            .unwrap_or(None);
        row.zahir = parse_zahir_value(zahir.as_deref());
    }

    if json {
        emit_json(&row)?;
    } else {
        println!("path:     {}", row.path);
        println!("category: {}", row.category);
        println!("size:     {} ({})", row.size, format_bytes(row.size));
        if include_zahir {
            match row.zahir {
                Some(ref v) => {
                    println!("zahir:");
                    println!("{}", serde_json::to_string_pretty(v)?);
                }
                None => println!("zahir:    (empty)"),
            }
        }
    }
    Ok(())
}

fn print_entries(conn: &Connection, args: &QueryCli) -> Result<(), anyhow::Error> {
    let mut rows = load_entries(conn, args.category.as_deref())?;
    if let Some(min) = args.min_size {
        rows.retain(|r| r.size >= min);
    }
    if let Some(max) = args.max_size {
        rows.retain(|r| r.size <= max);
    }
    if let Some(ref needle) = args.contains {
        rows.retain(|r| r.path.contains(needle.as_str()));
    }
    emit_entries(&rows, args.json)
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

fn emit_entries(rows: &[EntryRow], json: bool) -> Result<(), anyhow::Error> {
    if json {
        return emit_json(rows);
    }
    println!("{:<12} {:>10}  PATH", "CATEGORY", "SIZE");
    for r in rows {
        println!(
            "{:<12} {:>10}  {}",
            truncate(&r.category, 12),
            format_bytes(r.size),
            r.path
        );
    }
    eprintln!("{} entries", rows.len());
    Ok(())
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

fn validate_delta_type(t: &str) -> Result<(), anyhow::Error> {
    if DeltaType::iter().any(|d| d.as_str() == t) {
        Ok(())
    } else {
        anyhow::bail!("invalid --delta-type {t:?}; expected added|mod|removed")
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
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
