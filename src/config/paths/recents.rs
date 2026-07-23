//! Indexed-root recents scoring and welcome-prompt gates.

use std::collections::{HashSet, hash_map::Entry};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use log::debug;

use super::dirs::{cache_dir, db_dir};
use super::names::{UBLX_NAMES, hash_suffix_from_db_stem, is_hex_hash16, path_to_hex};
use super::ublx_paths::UblxPaths;

/// Per-indexed-dir metadata for welcome-screen recents: `cache_dir()/recents/<path_hash>.txt`.
const RECENTS_SUBDIR: &str = "recents";

/// Weight for [`times_opened`] in [`recents_composite_score`]: each session open adds this many
/// effective nanoseconds so frequently opened roots stay competitive vs raw `last_open_ns`.
const RECENTS_OPEN_WEIGHT_NS: u128 = 3_600_000_000_000; // 1 hour per open

#[must_use]
fn recents_dir() -> Option<PathBuf> {
    cache_dir().map(|c| c.join(RECENTS_SUBDIR))
}

#[must_use]
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

#[derive(Debug, Clone)]
struct RecentsFileData {
    path: PathBuf,
    times_opened: u64,
    last_open_ns: u64,
}

fn fmt_recents_txt(data: &RecentsFileData) -> String {
    format!(
        "path={}\ntimes_opened={}\nlast_open_ns={}\n",
        data.path.to_string_lossy(),
        data.times_opened,
        data.last_open_ns
    )
}

fn parse_recents_txt(content: &str) -> Option<RecentsFileData> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.contains('=') {
        let p = PathBuf::from(trimmed);
        return Some(RecentsFileData {
            path: p,
            times_opened: 0,
            last_open_ns: 0,
        });
    }
    let mut path: Option<PathBuf> = None;
    let mut times_opened: u64 = 0;
    let mut last_open_ns: u64 = 0;
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        match k.trim() {
            "path" => path = Some(PathBuf::from(v.trim())),
            "times_opened" => times_opened = v.trim().parse().unwrap_or(0),
            "last_open_ns" => last_open_ns = v.trim().parse().unwrap_or(0),
            _ => {}
        }
    }
    path.map(|p| RecentsFileData {
        path: p,
        times_opened,
        last_open_ns,
    })
}

fn read_recents_file(path: &Path) -> Option<RecentsFileData> {
    let s = fs::read_to_string(path).ok()?;
    parse_recents_txt(&s)
}

/// Composite ordering: mostly `last_open_ns`, with a boost from `times_opened`.
#[must_use]
fn recents_composite_score(data: &RecentsFileData) -> u128 {
    u128::from(data.last_open_ns)
        .saturating_add(u128::from(data.times_opened).saturating_mul(RECENTS_OPEN_WEIGHT_NS))
}

/// True if `cache_dir()/recents/{path_hash(dir)}.txt` exists (this root was registered after the welcome flow).
#[must_use]
pub fn has_recents_entry_for_dir(dir: &Path) -> bool {
    let Some(recents) = recents_dir() else {
        return false;
    };
    let key = path_to_hex(dir);
    recents.join(format!("{key}.txt")).exists()
}

/// Whether to show the first-run welcome UI for this indexed root.
///
/// **Product rule:** when not in headless snapshot mode, show if the per-root `SQLite` file under [`UblxPaths::db`]
/// (in `cache_dir()/ubli/`) did **not** exist yet **before** [`crate::engine::db_ops::ensure_ublx_and_db`].
/// Recents and local `ublx.toml` are **not** part of this gate.
///
/// Callers should compute `had_ubli_db_file` with `UblxPaths::new(dir).db().exists()` **before**
/// `ensure_ublx_and_db` (same order as [`crate::main`]).
#[must_use]
pub fn should_show_initial_prompt(
    snapshot_only: bool,
    had_index_db_before_ensure: bool,
    had_any_cached_db_before_this_root: bool,
) -> bool {
    let initial_prompt = !snapshot_only && !had_index_db_before_ensure;
    debug!(
        "initial_prompt={initial_prompt} (had_index_db_before_ensure={had_index_db_before_ensure})"
    );
    debug!("cached ublx roots seen before startup: {had_any_cached_db_before_this_root}");
    initial_prompt
}

/// True when the shared `ubli` directory contains at least one DB file.
#[must_use]
pub fn has_any_cached_ublx_db() -> bool {
    let Some(dir) = db_dir() else {
        return false;
    };
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    rd.flatten().any(|e| {
        e.path()
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(UBLX_NAMES.index_db_file_ext))
    })
}

/// Register this root after first-run **UBLX here**: creates or updates `recents` entry (path, `last_open_ns`; `times_opened` starts at 0 and is incremented by [`record_ublx_session_open`] on each post-prompt session).
///
/// # Errors
///
/// Returns an error if the recents directory cannot be created or the recents file cannot be written.
pub fn remember_indexed_root_path(dir: &Path) -> Result<()> {
    let Some(recents) = recents_dir() else {
        return Ok(());
    };
    fs::create_dir_all(&recents)?;
    let key = path_to_hex(dir);
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let path_file = recents.join(format!("{key}.txt"));
    let mut data = read_recents_file(&path_file).unwrap_or(RecentsFileData {
        path: canon.clone(),
        times_opened: 0,
        last_open_ns: 0,
    });
    data.path = canon;
    data.last_open_ns = now_ns();
    fs::write(path_file, fmt_recents_txt(&data))?;
    Ok(())
}

/// Refresh `last_open_ns` when the user picks a prior root from the welcome list (does not create a file).
/// Session `times_opened` is updated when the new process runs [`record_ublx_session_open`].
///
/// # Errors
///
/// Returns an error if the recents file cannot be written.
pub fn record_prior_root_selected(dir: &Path) -> Result<()> {
    let Some(recents) = recents_dir() else {
        return Ok(());
    };
    let key = path_to_hex(dir);
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let path_file = recents.join(format!("{key}.txt"));
    if !path_file.exists() {
        return Ok(());
    }
    let Some(mut data) = read_recents_file(&path_file) else {
        return Ok(());
    };
    data.path = canon;
    data.last_open_ns = now_ns();
    fs::write(path_file, fmt_recents_txt(&data))?;
    Ok(())
}

/// Each normal TUI session for a root that already has a recents file: increment `times_opened`, refresh `last_open_ns`.
/// Does not create a file (first registration is only via [`remember_indexed_root_path`] after **UBLX here**).
///
/// # Errors
///
/// Returns an error if the recents file cannot be written.
pub fn record_ublx_session_open(dir: &Path) -> Result<()> {
    let Some(recents) = recents_dir() else {
        return Ok(());
    };
    let key = path_to_hex(dir);
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let path_file = recents.join(format!("{key}.txt"));
    if !path_file.exists() {
        return Ok(());
    }
    let Some(mut data) = read_recents_file(&path_file) else {
        return Ok(());
    };
    data.path = canon;
    data.times_opened = data.times_opened.saturating_add(1);
    data.last_open_ns = now_ns();
    fs::write(path_file, fmt_recents_txt(&data))?;
    Ok(())
}

/// Collect all recents entries (deduped by canonical path).
fn collect_recents_entries() -> Vec<RecentsFileData> {
    let Some(dir) = recents_dir() else {
        return Vec::new();
    };
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut best: std::collections::HashMap<PathBuf, RecentsFileData> =
        std::collections::HashMap::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let Some(mut data) = read_recents_file(&p) else {
            continue;
        };
        let canon = data
            .path
            .canonicalize()
            .unwrap_or_else(|_| data.path.clone());
        data.path.clone_from(&canon);
        match best.entry(canon) {
            Entry::Occupied(mut o) => {
                let ex = o.get_mut();
                if data.last_open_ns > ex.last_open_ns
                    || (data.last_open_ns == ex.last_open_ns && data.times_opened > ex.times_opened)
                {
                    *ex = data;
                }
            }
            Entry::Vacant(v) => {
                v.insert(data);
            }
        }
    }
    best.into_values().collect()
}

/// Indexed roots that have **both** a valid `recents/{hash}.txt` and the matching **main** DB under `ubli/`.
///
/// For each recents file, the path in the file must resolve to a directory whose expected DB exists, and the
/// **16-hex hash in the recents filename** must match the **hash suffix** of that DB file’s stem (same rule as
/// [`UblxPaths::db_stem`]). Entries with a missing DB, wrong hash, or non-hex filename are skipped.
#[must_use]
pub fn all_indexed_roots_alphabetical() -> Vec<PathBuf> {
    let Some(recents) = recents_dir() else {
        return Vec::new();
    };
    let Ok(rd) = fs::read_dir(&recents) else {
        return Vec::new();
    };

    let mut out: HashSet<PathBuf> = HashSet::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let Some(fname) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_hex_hash16(fname) {
            continue;
        }
        let Some(data) = read_recents_file(&p) else {
            continue;
        };
        let path = data
            .path
            .canonicalize()
            .unwrap_or_else(|_| data.path.clone());
        if !path.is_dir() {
            continue;
        }
        let db_path = UblxPaths::new(&path).db();
        if !db_path.exists() {
            continue;
        }
        let Some(db_stem) = db_path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(hash_from_db) = hash_suffix_from_db_stem(db_stem) else {
            continue;
        };
        if hash_from_db != fname {
            continue;
        }
        out.insert(path);
    }

    let mut paths: Vec<PathBuf> = out.into_iter().collect();
    paths.sort_by_key(|a| a.display().to_string());
    paths
}

/// Prior indexed roots that still look valid (directory exists and has a DB file), excluding `current`.
#[must_use]
pub fn prior_indexed_roots(current: &Path) -> Vec<PathBuf> {
    prior_indexed_roots_scored(current, usize::MAX)
        .into_iter()
        .map(|(p, _)| p)
        .collect()
}

/// Scoring prior indexed roots based on time last opened and times opened
fn prior_indexed_roots_scored(current: &Path, max: usize) -> Vec<(PathBuf, RecentsFileData)> {
    let current_canon = current
        .canonicalize()
        .unwrap_or_else(|_| current.to_path_buf());
    let mut scored: Vec<(PathBuf, RecentsFileData)> = Vec::new();
    for mut data in collect_recents_entries() {
        let dir = data
            .path
            .canonicalize()
            .unwrap_or_else(|_| data.path.clone());
        if dir == current_canon || !dir.is_dir() {
            continue;
        }
        let db = UblxPaths::new(&dir).db();
        if !db.exists() {
            continue;
        }
        data.path.clone_from(&dir);
        scored.push((dir, data));
    }
    scored.sort_by(|a, b| {
        recents_composite_score(&b.1)
            .cmp(&recents_composite_score(&a.1))
            .then_with(|| a.0.cmp(&b.0))
    });
    scored.truncate(max);
    scored
}

/// Same as [`prior_indexed_roots`], but sorted by [`recents_composite_score`], capped.
#[must_use]
pub fn prior_indexed_roots_recent(current: &Path, max: usize) -> Vec<PathBuf> {
    prior_indexed_roots_scored(current, max)
        .into_iter()
        .map(|(p, _)| p)
        .collect()
}
