//! Shared stdout helpers for headless CLI subcommands (`query`, `doctor`).

use serde::Serialize;

/// Pretty-print `value` as JSON to stdout.
///
/// # Errors
///
/// Returns `Err` when serialization fails.
pub fn emit_json<T: Serialize + ?Sized>(value: &T) -> Result<(), anyhow::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// Print one string per line, then a count footer on stderr (human mode).
pub fn emit_string_list(items: &[String], noun: &str, json: bool) -> Result<(), anyhow::Error> {
    if json {
        return emit_json(items);
    }
    if items.is_empty() {
        eprintln!("(no {noun})");
    } else {
        for item in items {
            println!("{item}");
        }
        eprintln!("{} {noun}", items.len());
    }
    Ok(())
}
