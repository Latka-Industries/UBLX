//! Flat on-disk export of snapshot `zahir_json` (headless `--export`).

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Context;
use serde_json::Value;

use crate::paths::UBLX_NAMES;

use super::load_snapshot_zahir_json_map;

/// Write each non-empty snapshot `zahir_json` to `dir_to_ublx/{zahir_export_dir_name}/` as a flat file.
/// JSON is pretty-printed when valid; otherwise bytes are written as stored.
/// Each filename is derived from the indexed relative path: `\` and `/` become `_`, other non-safe
/// characters become `_`, then `.json` is appended. If two paths map to the same stem, `__2`, `__3`, …
/// are inserted before `.json`.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on I/O or DB errors.
pub fn export_zahir_json_flat(dir_to_ublx: &Path, db_path: &Path) -> Result<usize, anyhow::Error> {
    let map = load_snapshot_zahir_json_map(db_path)?;
    let out_dir = dir_to_ublx.join(UBLX_NAMES.zahir_export_dir_name);
    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir.display()))?;

    let mut taken = HashSet::<String>::new();
    let mut count = 0usize;
    for (rel_path, json) in map {
        let fname = unique_flat_json_name(&rel_path, &mut taken);
        let path = out_dir.join(&fname);
        let body = zahir_json_pretty_or_raw(&json);
        fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
        count += 1;
    }
    Ok(count)
}

fn zahir_json_pretty_or_raw(s: &str) -> String {
    match serde_json::from_str::<Value>(s) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| s.to_string()) + "\n",
        Err(_) => s.to_string(),
    }
}

fn flat_stem_from_rel_path(rel_path: &str) -> String {
    let s = rel_path.trim().replace('\\', "/");
    let s = s.trim_start_matches("./");
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '/' => out.push('_'),
            c if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' => out.push(c),
            _ => out.push('_'),
        }
    }
    if out.is_empty() {
        "root".to_string()
    } else {
        out
    }
}

fn unique_flat_json_name(rel_path: &str, taken: &mut HashSet<String>) -> String {
    let stem = flat_stem_from_rel_path(rel_path);
    let mut candidate = format!("{stem}.json");
    if !taken.contains(&candidate) {
        taken.insert(candidate.clone());
        return candidate;
    }
    let mut n = 2u32;
    loop {
        candidate = format!("{stem}__{n}.json");
        if !taken.contains(&candidate) {
            taken.insert(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}
