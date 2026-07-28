//! Snapshot DB path resolution for reading. Use when the TUI should show live data
//! from a running snapshot (e.g. read from the temp file until rename to the final [`INDEX_DB_FILE_EXT`] file).

use std::path::{Path, PathBuf};

use crate::paths::UBLX_NAMES;

fn tmp_from_db_path(db_path: &Path) -> Option<PathBuf> {
    let parent = db_path.parent()?;
    let file_name = db_path.file_name()?.to_str()?;
    if let Some(stem) = file_name.strip_suffix(UBLX_NAMES.index_db_file_ext) {
        Some(parent.join(format!("{stem}_tmp{}", UBLX_NAMES.index_db_file_ext)))
    } else {
        // Pre-extension layout: final `stem`, temp `stem_tmp` (no [`INDEX_DB_FILE_EXT`]).
        Some(parent.join(format!("{file_name}_tmp")))
    }
}

/// Which file to prefer when both exist: final `.ublx` or in-progress `.ublx_tmp`.
#[derive(Clone, Copy)]
pub enum SnapshotReaderPreference {
    /// Prefer `.ublx`; use `.ublx_tmp` only when `.ublx` is missing (e.g. first run).
    PreferUblx,
    /// Prefer `.ublx_tmp` so the TUI shows live data while a pipeline is running.
    PreferTmp,
}

/// Path to use for reading the snapshot. Returns `None` when neither `.ublx` nor `.ublx_tmp` exists.
#[must_use]
pub fn snapshot_reader_path_with(
    db_path: &Path,
    preference: SnapshotReaderPreference,
) -> Option<PathBuf> {
    let db = db_path.to_path_buf();
    let tmp = tmp_from_db_path(db_path)?;
    let (first, second) = match preference {
        SnapshotReaderPreference::PreferUblx => (db, tmp),
        SnapshotReaderPreference::PreferTmp => (tmp, db),
    };
    if first.exists() {
        Some(first)
    } else if second.exists() {
        Some(second)
    } else {
        None
    }
}

/// Path the TUI should use for reading snapshot data (list rows, `zahir_json`, mtime). When `prefer_tmp` is true (e.g. snapshot still running), prefers `.ublx_tmp` so the UI shows live progress; otherwise prefers `.ublx`. Falls back to `db_path` if neither exists.
#[must_use]
pub fn snapshot_read_path_for_tui(db_path: &Path, prefer_tmp: bool) -> PathBuf {
    let preference = if prefer_tmp {
        SnapshotReaderPreference::PreferTmp
    } else {
        SnapshotReaderPreference::PreferUblx
    };
    snapshot_reader_path_with(db_path, preference).unwrap_or_else(|| db_path.to_path_buf())
}
