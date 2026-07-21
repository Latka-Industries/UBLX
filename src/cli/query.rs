//! `ublx query` — headless catalog query (THI-153); optional remote via `--url` / `UBLX_URL` (THI-167).

use crate::cli::catalog::open_catalog_for_read;
use crate::cli::catalog_read::{
    DeltaRow, EntryListFilter, EntryRow, entry_detail, list_categories, list_delta, list_entries,
    list_lens_entries, list_lens_names,
};
use crate::cli::output::{emit_json, emit_string_list};
use crate::cli::remote::{encode_entry_path, get_json, path_with_query, resolve_base};
use crate::cli_parser::QueryCli;
use crate::utils::format_bytes;

/// Run `ublx query` against the local catalog for `DIR`, or a remote `ublx serve` when `--url` is set.
///
/// # Errors
///
/// Returns `Err` when the directory/catalog cannot be opened, the remote request fails, or a query fails.
pub fn run(args: &QueryCli) -> Result<(), anyhow::Error> {
    if let Some(base) = resolve_base(args.remote.url.as_deref()) {
        return run_remote(&base, args);
    }
    run_local(args)
}

fn run_local(args: &QueryCli) -> Result<(), anyhow::Error> {
    let handle = open_catalog_for_read(&args.dir)?;
    let conn = &handle.conn;

    if args.categories {
        return emit_string_list(&list_categories(conn)?, "categories", args.json);
    }
    if args.lenses {
        return emit_string_list(&list_lens_names(conn)?, "lenses", args.json);
    }
    if let Some(ref name) = args.lens {
        return emit_entries(&list_lens_entries(conn, name)?, args.json);
    }
    if args.delta {
        return emit_delta(&list_delta(conn, args.delta_type.as_deref())?, args.json);
    }
    if let Some(ref path) = args.path {
        return emit_entry_detail(
            &entry_detail(conn, path, args.zahir)?,
            args.zahir,
            args.json,
        );
    }

    emit_entries(&list_entries(conn, &entry_filter(args))?, args.json)
}

fn run_remote(base: &str, args: &QueryCli) -> Result<(), anyhow::Error> {
    if args.categories {
        let rows: Vec<String> = get_json(base, "/categories")?;
        return emit_string_list(&rows, "categories", args.json);
    }
    if args.lenses {
        let rows: Vec<String> = get_json(base, "/lenses")?;
        return emit_string_list(&rows, "lenses", args.json);
    }
    if let Some(ref name) = args.lens {
        let path = format!("/lenses/{}", encode_entry_path(name));
        let rows: Vec<EntryRow> = get_json(base, &path)?;
        return emit_entries(&rows, args.json);
    }
    if args.delta {
        let path = match args.delta_type.as_deref() {
            Some(t) => path_with_query("/delta", &[("type", t)]),
            None => "/delta".to_owned(),
        };
        let rows: Vec<DeltaRow> = get_json(base, &path)?;
        return emit_delta(&rows, args.json);
    }
    if let Some(ref path) = args.path {
        let entry_path = format!("/entries/{}", encode_entry_path(path));
        let pq = if args.zahir {
            path_with_query(&entry_path, &[("zahir", "1")])
        } else {
            entry_path
        };
        let row: EntryRow = get_json(base, &pq)?;
        return emit_entry_detail(&row, args.zahir, args.json);
    }

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
    let rows: Vec<EntryRow> = get_json(base, &path_with_query("/entries", &refs))?;
    emit_entries(&rows, args.json)
}

fn entry_filter(args: &QueryCli) -> EntryListFilter<'_> {
    EntryListFilter::new(
        args.category.as_deref(),
        args.min_size,
        args.max_size,
        args.contains.as_deref(),
    )
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
