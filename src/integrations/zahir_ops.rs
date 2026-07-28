//! `ZahirScan` integration: batch (sequential) and stream entry points, plus Zarr path collapsing.
//!
//! File-type hints and `zahir_json` serialization moved to `ublx_catalog::zahir` (the crate that writes
//! those snapshot columns) and are re-exported here.

use std::collections::{HashMap, hash_map::Entry};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use log::debug;
use zahirscan;

use crate::config::UblxOpts;
use crate::utils::path_to_slash_string;

use super::nefax_ops;

pub type ZahirOutputSink = zahirscan::OutputSink;
pub type ZahirOutputMode = zahirscan::OutputMode;
pub type ZahirRC = zahirscan::RuntimeConfig;

/// Snapshot-facing aliases and the pure file-type / `zahir_json` helpers are owned by `ublx-catalog`
/// (it writes those columns); the `extract_zahir` runners below stay here.
pub use ublx_catalog::{
    ZahirFT, ZahirOutput, ZahirResult, file_type_from_metadata_name, get_zahir_output_by_path,
    zahir_metadata_name_from_indexed_file, zahir_metadata_name_from_path_hint,
    zahir_output_to_json, zahir_output_to_json_for_path,
};

/// Safe ffprobe invocation (JSON format/streams). Delegates to [`zahirscan::utils::ffprobe_handler::run_ffprobe_safe`].
pub use zahirscan::utils::ffprobe_handler::run_ffprobe_safe;

/// `true` when `s` is the zahir display name for Zarr (e.g. snapshot `category` for a Zarr store).
#[inline]
#[must_use]
pub fn is_zarr_category_str(s: &str) -> bool {
    file_type_from_metadata_name(s) == Some(ZahirFT::Zarr)
}

/// Sniff delimiter from the first lines of `content` (comma, semicolon, tab, pipe, colon).
/// Use as a **fallback** when the file path has no recognized extension (see [`delimiter_from_path_for_viewer`]).
#[must_use]
pub fn detect_delimiter_byte(content: &str) -> u8 {
    zahirscan::parsers::structured::detect_delimiter_byte(content)
}

/// Delimiter implied by the path’s extension, when it matches zahirscan’s delimited types.
/// `.csv` → comma, `.tsv` / `.tab` → tab, `.psv` → pipe; otherwise [`None`] (caller should use [`detect_delimiter_byte`]).
#[must_use]
pub fn delimiter_from_path_for_viewer(path: &str) -> Option<u8> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)?;
    match ext.as_str() {
        "csv" => Some(b','),
        "tsv" | "tab" => Some(b'\t'),
        "psv" => Some(b'|'),
        _ => None,
    }
}

// --- extract_zahir entry points ---------------------------------------------

/// True if we should run zahir on this path (new or mtime changed). Skip when prior exists and mtime is unchanged.
#[must_use]
pub fn needs_zahir(
    prior_nefax: Option<&nefax_ops::NefaxResult>,
    path: &PathBuf,
    current_mtime_ns: i64,
) -> bool {
    match prior_nefax.and_then(|p| p.get(path)) {
        Some(prior_meta) => prior_meta.mtime_ns != current_mtime_ns,
        None => true,
    }
}

/// When `paths` is empty, log and return a default result so callers skip `extract_zahir`.
fn zahir_empty_when_no_paths(
    paths: &[String],
    mode_label: &'static str,
) -> Option<zahirscan::ZahirScanResult> {
    if paths.is_empty() {
        debug!("zahir {mode_label}: no paths received, returning empty result");
        Some(zahirscan::ZahirScanResult::default())
    } else {
        None
    }
}

/// Run zahir on a full set of paths (sequential mode). Uses [`OutputMode::Full`] and the given config.
///
/// If the path list is empty, returns [`ZahirScanResult::default`] without calling zahirscan.
///
/// # Errors
///
/// Returns [`anyhow::Error`] when zahirscan fails (including when no paths are scannable).
pub fn run_zahir_batch(
    paths: &[impl AsRef<Path>],
    ublx_opts: &UblxOpts,
) -> Result<zahirscan::ZahirScanResult, anyhow::Error> {
    let config = ublx_opts.zahir_runtime_config();
    let path_strings: Vec<String> = paths
        .iter()
        .map(|p| p.as_ref().to_string_lossy().into_owned())
        .collect();
    if let Some(empty) = zahir_empty_when_no_paths(&path_strings, "batch") {
        return Ok(empty);
    }
    zahirscan::extract_zahir(
        path_strings,
        config.output_mode,
        Some(&config),
        None,
        &ZahirOutputSink::Collect,
    )
}

/// Run zahir on paths from a channel. Drains `paths_rx` until closed (same as [`zahirscan::extract_zahir_from_stream`]), then runs [`extract_zahir`].
/// Use `ZahirOutputSink::Collect` to get all outputs in the result (default).
/// Use `ZahirOutputSink::Channel(tx)` to stream each `(path, Output)` to a receiver so ublx can write to the DB incrementally.
///
/// If no paths were received, returns [`ZahirScanResult::default`] without calling zahirscan.
///
/// # Errors
///
/// Returns [`anyhow::Error`] when zahirscan fails (including when no paths are scannable).
pub fn run_zahir_from_stream(
    paths_rx: &Receiver<String>,
    ublx_opts: &UblxOpts,
    output_sink: &ZahirOutputSink,
) -> Result<zahirscan::ZahirScanResult, anyhow::Error> {
    let config = ublx_opts.zahir_runtime_config();
    let path_strings: Vec<String> = paths_rx.iter().collect();
    if let Some(empty) = zahir_empty_when_no_paths(&path_strings, "stream") {
        return Ok(empty);
    }
    zahirscan::extract_zahir(
        path_strings,
        config.output_mode,
        Some(&config),
        None,
        output_sink,
    )
}

/// Keep only Zarr **store root** paths (drop `…/store.zarr/…` inners). Delegates to zahirscan
/// [`filter_zarr_input_paths`](zahirscan::utils::zarr_paths::filter_zarr_input_paths).
#[must_use]
pub fn zahir_zarr_path_filter(paths: &[impl AsRef<Path>]) -> Vec<String> {
    let path_strings: Vec<String> = paths
        .iter()
        .map(|p| p.as_ref().to_string_lossy().into_owned())
        .collect();
    zahirscan::utils::zarr_paths::filter_zarr_input_paths(path_strings)
}

/// Filesystem path collapsed to the Zarr store root (directory whose name ends with `.zarr`), if any
/// prefix of `path` is such a store; otherwise `None`.
#[must_use]
pub fn zarr_collapse_to_store_root_path(path: &Path) -> Option<PathBuf> {
    let ext = zahirscan::utils::zarr_paths::ZARR_EXTENSION;
    let mut acc = PathBuf::new();
    for c in path.components() {
        acc.push(c);
        if c.as_os_str().to_str().is_some_and(|s| s.ends_with(ext)) {
            return Some(acc);
        }
    }
    None
}

/// [`zarr_collapse_to_store_root_path`] or `path` unchanged when not under a `.zarr` store.
#[must_use]
pub fn zarr_collapse_to_store_root_path_or_same(path: &Path) -> PathBuf {
    zarr_collapse_to_store_root_path(path).unwrap_or_else(|| path.to_path_buf())
}

/// Relative path (slash-normalized) of the Zarr **store root** for this path, if it lies inside or on a
/// `*.zarr` store; `None` if the path does not reference `.zarr`.
#[must_use]
fn zarr_store_root_path_str(s: &str) -> Option<String> {
    let t = s.trim().trim_end_matches(['/', '\\']);
    if !t.contains(zahirscan::utils::zarr_paths::ZARR_EXTENSION) {
        return None;
    }
    if zahirscan::utils::zarr_paths::is_zarr_store_root_path(t) {
        return Some(t.replace('\\', "/"));
    }
    let parts: Vec<&str> = t.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    for i in 0..parts.len() {
        if parts[i].ends_with(zahirscan::utils::zarr_paths::ZARR_EXTENSION) {
            return Some(parts[..=i].join("/"));
        }
    }
    None
}

fn merge_nefax_path_meta(into: &mut nefax_ops::NefaxPathMeta, add: &nefax_ops::NefaxPathMeta) {
    into.mtime_ns = into.mtime_ns.max(add.mtime_ns);
    into.size = into.size.saturating_add(add.size);
    into.hash = None;
}

/// True if this **relative** path (as nefaxer keys it) is a Zarr store root (`…/name.zarr` with no
/// path after the store segment). Used to still run batch zahir on a directory-backed `.zarr` store.
#[must_use]
pub fn is_zarr_store_root_rel_path(path: &Path) -> bool {
    let s = path_to_slash_string(path);
    zahirscan::utils::zarr_paths::is_zarr_store_root_path(s.trim().trim_end_matches(['/', '\\']))
}

/// Collapse Nefax entries so only one row per Zarr store root is kept (inners under `name.zarr/` are
/// merged into the root’s [`NefaxPathMeta`]: max `mtime_ns`, sum `size`, `hash` cleared if merged).
#[must_use]
pub fn nefax_collapse_zarr_inners(nefax: nefax_ops::NefaxResult) -> nefax_ops::NefaxResult {
    let mut out: nefax_ops::NefaxResult = HashMap::new();
    for (path, meta) in nefax {
        let s = path_to_slash_string(&path);
        let key = if let Some(root) = zarr_store_root_path_str(&s) {
            PathBuf::from(root)
        } else {
            path
        };
        match out.entry(key) {
            Entry::Vacant(e) => {
                e.insert(meta);
            }
            Entry::Occupied(mut e) => {
                merge_nefax_path_meta(e.get_mut(), &meta);
            }
        }
    }
    out
}

/// Map added/modified/removed paths the same way as [`nefax_collapse_zarr_inners`], deduplicating
/// so `delta_log` row counts match the collapsed snapshot.
#[must_use]
pub fn nefax_diff_collapse_zarr_inners(diff: nefax_ops::NefaxDiff) -> nefax_ops::NefaxDiff {
    nefax_ops::NefaxDiff {
        added: dedupe_collapse_path_list(diff.added),
        removed: dedupe_collapse_path_list(diff.removed),
        modified: dedupe_collapse_path_list(diff.modified),
    }
}

fn dedupe_collapse_path_list(list: Vec<PathBuf>) -> Vec<PathBuf> {
    use std::collections::HashSet;
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut out = Vec::new();
    for p in list {
        let s = path_to_slash_string(&p);
        let p = if let Some(root) = zarr_store_root_path_str(&s) {
            PathBuf::from(root)
        } else {
            p
        };
        if seen.insert(p.clone()) {
            out.push(p);
        }
    }
    out
}
