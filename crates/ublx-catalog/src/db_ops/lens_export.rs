//! On-disk Markdown export of lenses: one `.md` file per lens under `dir_to_ublx/{lens_export_dir_name}/`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::paths::UBLX_NAMES;
use crate::zahir::{ZahirFT, file_type_from_metadata_name};

use super::SnapshotTuiRow;
use super::lens_storage::{load_lens_names, load_lens_paths};

/// Write one Markdown file per lens with `#` title and per-path links relative to each file.
/// Entries are separated by a blank line; images use `![path](url)`; others use `[path](url)`.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on I/O or DB errors.
pub fn export_lenses_markdown_flat(
    dir_to_ublx: &Path,
    db_path: &Path,
) -> Result<usize, anyhow::Error> {
    if !db_path.exists() {
        return Ok(0);
    }
    let lens_names = load_lens_names(db_path)?;
    if lens_names.is_empty() {
        return Ok(0);
    }
    let out_dir = dir_to_ublx.join(UBLX_NAMES.lens_export_dir_name);
    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir.display()))?;

    let mut taken = HashSet::<String>::new();
    let mut count = 0usize;
    for name in lens_names {
        let rows = load_lens_paths(db_path, &name)?;
        let fname = unique_flat_md_name(&name, &mut taken);
        let md_path = out_dir.join(&fname);
        let body = build_lens_markdown_body(dir_to_ublx, &md_path, &name, &rows);
        fs::write(&md_path, body).with_context(|| format!("write {}", md_path.display()))?;
        count += 1;
    }
    Ok(count)
}

fn build_lens_markdown_body(
    dir_to_ublx: &Path,
    md_path: &Path,
    lens_name: &str,
    rows: &[SnapshotTuiRow],
) -> String {
    let mut s = String::new();
    s.push_str("# ");
    s.push_str(lens_name.trim().replace('\n', " ").as_str());
    s.push_str("\n\n");
    for (i, (rel_path, category, _)) in rows.iter().enumerate() {
        if i > 0 {
            s.push_str("\n\n");
        }
        let path_display = rel_path.trim().replace('\\', "/");
        let esc = md_escape_brackets(&path_display);
        let url = rel_url_from_md_to_target(md_path, dir_to_ublx, rel_path.as_str());
        let target = md_link_target(&url);
        if is_image_category(category) {
            s.push_str("![");
            s.push_str(&esc);
            s.push_str("](");
            s.push_str(target.as_str());
            s.push(')');
        } else {
            s.push('[');
            s.push_str(&esc);
            s.push_str("](");
            s.push_str(target.as_str());
            s.push(')');
        }
    }
    s.push('\n');
    s
}

fn md_escape_brackets(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' | '[' | ']' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn is_image_category(category: &str) -> bool {
    file_type_from_metadata_name(category) == Some(ZahirFT::Image)
}

fn md_link_target(url: &str) -> String {
    if url.contains(' ') || url.contains('(') {
        format!("<{url}>")
    } else {
        url.to_string()
    }
}

fn rel_url_from_md_to_target(md_file: &Path, dir_to_ublx: &Path, rel_path: &str) -> String {
    let rel = rel_path.trim().replace('\\', "/");
    let target = dir_to_ublx.join(&rel);
    let md_dir = md_file.parent().unwrap_or_else(|| Path::new("."));
    path_relative_to(md_dir, &target)
        .to_string_lossy()
        .replace('\\', "/")
}

fn path_relative_to(base: &Path, full: &Path) -> PathBuf {
    let base_c: Vec<_> = base.components().collect();
    let path_c: Vec<_> = full.components().collect();
    let mut i = 0;
    let n = base_c.len().min(path_c.len());
    while i < n && base_c[i] == path_c[i] {
        i += 1;
    }
    let mut out = PathBuf::new();
    for _ in i..base_c.len() {
        out.push("..");
    }
    for c in path_c.iter().skip(i) {
        out.push(c);
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

fn flat_stem_from_lens_name(name: &str) -> String {
    let s = name.trim().replace('\\', "/");
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '/' => out.push('_'),
            c if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' => out.push(c),
            _ => out.push('_'),
        }
    }
    if out.is_empty() {
        "lens".to_string()
    } else {
        out
    }
}

fn unique_flat_md_name(lens_name: &str, taken: &mut HashSet<String>) -> String {
    let stem = flat_stem_from_lens_name(lens_name);
    let mut candidate = format!("{stem}.md");
    if !taken.contains(&candidate) {
        taken.insert(candidate.clone());
        return candidate;
    }
    let mut n = 2u32;
    loop {
        candidate = format!("{stem}__{n}.md");
        if !taken.contains(&candidate) {
            taken.insert(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}
