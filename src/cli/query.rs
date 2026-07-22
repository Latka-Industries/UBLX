//! `ublx query` — headless catalog query (THI-153); optional remote via `--url` / `UBLX_URL` (THI-167).

use rusqlite::Connection;

use crate::cli::catalog::open_catalog_for_read;
use crate::cli::catalog_read::{
    DeltaRow, EntryListFilter, EntryRow, entry_detail, list_categories, list_delta, list_entries,
    list_lens_entries, list_lens_names,
};
use crate::cli::output::{emit_json, emit_string_list};
use crate::cli::remote::{encode_entry_path, get_json, path_with_query, resolve_base};
use crate::cli_parser::QueryCli;
use crate::utils::{format_bytes, format_timestamp_ns};

/// Run `ublx query` against the local catalog for `DIR`, or a remote `ublx serve` when `--url` is set.
///
/// # Errors
///
/// Returns `Err` when the directory/catalog cannot be opened, the remote request fails, or a query fails.
pub fn run(args: &QueryCli) -> Result<(), anyhow::Error> {
    let result = if let Some(base) = resolve_base(args.remote.url.as_deref()) {
        collect_remote(&base, args)?
    } else {
        let handle = open_catalog_for_read(&args.dir)?;
        collect_local(&handle.conn, args)?
    };
    emit_result(&result, args.json)
}

enum QueryResult {
    Strings {
        items: Vec<String>,
        noun: &'static str,
    },
    Entries(Vec<EntryRow>),
    Delta(Vec<DeltaRow>),
    Detail {
        row: EntryRow,
        zahir: bool,
    },
}

fn collect_local(conn: &Connection, args: &QueryCli) -> Result<QueryResult, anyhow::Error> {
    if args.categories {
        return Ok(QueryResult::Strings {
            items: list_categories(conn)?,
            noun: "categories",
        });
    }
    if args.lenses {
        return Ok(QueryResult::Strings {
            items: list_lens_names(conn)?,
            noun: "lenses",
        });
    }
    if let Some(ref name) = args.lens {
        return Ok(QueryResult::Entries(list_lens_entries(conn, name)?));
    }
    if args.delta {
        return Ok(QueryResult::Delta(list_delta(
            conn,
            args.delta_type.as_deref(),
        )?));
    }
    if let Some(ref path) = args.path {
        return Ok(QueryResult::Detail {
            row: entry_detail(conn, path, args.zahir)?,
            zahir: args.zahir,
        });
    }
    Ok(QueryResult::Entries(list_entries(
        conn,
        &entry_filter(args),
    )?))
}

fn collect_remote(base: &str, args: &QueryCli) -> Result<QueryResult, anyhow::Error> {
    if args.categories {
        return Ok(QueryResult::Strings {
            items: get_json(base, "/categories")?,
            noun: "categories",
        });
    }
    if args.lenses {
        return Ok(QueryResult::Strings {
            items: get_json(base, "/lenses")?,
            noun: "lenses",
        });
    }
    if let Some(ref name) = args.lens {
        let path = format!("/lenses/{}", encode_entry_path(name));
        return Ok(QueryResult::Entries(get_json(base, &path)?));
    }
    if args.delta {
        let path = match args.delta_type.as_deref() {
            Some(t) => path_with_query("/delta", &[("type", t)]),
            None => "/delta".to_owned(),
        };
        return Ok(QueryResult::Delta(get_json(base, &path)?));
    }
    if let Some(ref path) = args.path {
        let entry_path = format!("/entries/{}", encode_entry_path(path));
        let pq = if args.zahir {
            path_with_query(&entry_path, &[("zahir", "1")])
        } else {
            entry_path
        };
        return Ok(QueryResult::Detail {
            row: get_json(base, &pq)?,
            zahir: args.zahir,
        });
    }
    Ok(QueryResult::Entries(get_json(
        base,
        &entries_list_path(args),
    )?))
}

fn entries_list_path(args: &QueryCli) -> String {
    let mut pairs: Vec<(&str, String)> = Vec::new();
    if let Some(ref c) = args.category {
        pairs.push(("category", c.clone()));
    }
    if let Some(n) = args.min_size {
        pairs.push(("min_size", n.to_string()));
    }
    if let Some(n) = args.max_size {
        pairs.push(("max_size", n.to_string()));
    }
    if let Some(ref c) = args.contains {
        pairs.push(("contains", c.clone()));
    }
    let refs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    path_with_query("/entries", &refs)
}

fn entry_filter(args: &QueryCli) -> EntryListFilter<'_> {
    EntryListFilter::new(
        args.category.as_deref(),
        args.min_size,
        args.max_size,
        args.contains.as_deref(),
    )
}

fn emit_result(result: &QueryResult, json: bool) -> Result<(), anyhow::Error> {
    match result {
        QueryResult::Strings { items, noun } => emit_string_list(items, noun, json),
        QueryResult::Entries(rows) => emit_entries(rows, json),
        QueryResult::Delta(rows) => emit_delta(rows, json),
        QueryResult::Detail { row, zahir } => emit_entry_detail(row, *zahir, json),
    }
}

fn emit_delta(rows: &[DeltaRow], json: bool) -> Result<(), anyhow::Error> {
    if json {
        emit_json(rows)?;
    } else {
        println!("{:<20} {:<8} PATH", "CREATED_NS", "TYPE");
        for r in rows {
            println!("{:<20} {:<8} {}", r.created_ns, r.delta_type, r.path);
        }
        eprintln!("{} delta rows", rows.len());
    }
    Ok(())
}

fn emit_entry_detail(row: &EntryRow, include_zahir: bool, json: bool) -> Result<(), anyhow::Error> {
    if json {
        emit_json(row)?;
    } else {
        println!("path:     {}", row.path);
        println!("category: {}", row.category);
        println!("size:     {} ({})", row.size, format_bytes(row.size));
        if let Some(ns) = row.mtime_ns {
            println!("mtime:    {}", format_timestamp_ns(ns));
        }
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
