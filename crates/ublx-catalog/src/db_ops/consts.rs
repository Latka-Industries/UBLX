use std::fs;
use std::path::Path;

use crate::paths::UblxPaths;
use crate::zahir::{ZahirFT, file_type_from_metadata_name};

/// Snapshot insert: prepare rows in parallel when the nefax result has at least this many paths.
///
/// Mirrors `ublx`'s `config::PARALLEL.snapshot_insert_prep` — kept here so the catalog crate does not
/// depend on the binary's tuning table. Change both together.
pub const SNAPSHOT_INSERT_PREP_PARALLEL: usize = 5000;

/// Schema for the ublx DB
pub struct UblxDbSchema;

impl UblxDbSchema {
    /// Snapshot table: one row per path. Nefaxer columns + ublx category + optional zahirscan JSON.
    pub const CREATE_SNAPSHOT: &'static str = "
CREATE TABLE IF NOT EXISTS snapshot (
    path TEXT PRIMARY KEY,
    mtime_ns INTEGER NOT NULL,
    size INTEGER NOT NULL,
    hash BLOB,
    category TEXT,
    zahir_json TEXT
);
";

    /// Settings table: single row storing disk/tuning so we can skip disk check when .ublx exists.
    /// `config_source`: 'local' | 'global' when global config exists; which config to use (stored in .ublx).
    pub const CREATE_SETTINGS: &'static str = "
CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    num_threads INTEGER NOT NULL,
    drive_type TEXT NOT NULL,
    parallel_walk INTEGER NOT NULL,
    config_source TEXT
);
";

    /// Delta log: one row per change (added, mod, removed); no zahir result, just path + meta.
    pub const CREATE_DELTA_LOG: &'static str = "
CREATE TABLE IF NOT EXISTS delta_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_ns INTEGER NOT NULL,
    path TEXT NOT NULL,
    mtime_ns INTEGER,
    size INTEGER,
    hash BLOB,
    delta_type TEXT NOT NULL CHECK (delta_type IN ('added', 'mod', 'removed'))
);
";

    /// Normalized path strings for lens membership (one row per distinct path).
    pub const CREATE_PATH: &'static str = "
CREATE TABLE IF NOT EXISTS path (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE
);
";

    /// Lenses (playlists): one row per lens, id + name.
    pub const CREATE_LENS: &'static str = "
CREATE TABLE IF NOT EXISTS lens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);
";

    /// Which paths are in which lens, and in what order (`lens_id`, `path_id`, position).
    pub const CREATE_LENS_PATH: &'static str = "
CREATE TABLE IF NOT EXISTS lens_path (
    lens_id INTEGER NOT NULL REFERENCES lens(id) ON DELETE CASCADE,
    path_id INTEGER NOT NULL REFERENCES path(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (lens_id, path_id)
);
CREATE INDEX IF NOT EXISTS idx_lens_path_lens_position ON lens_path (lens_id, position);
";

    /// SQL to create all ublx tables (snapshot, settings, `delta_log`, path, lens, `lens_path`). Use when opening or creating the DB.
    #[must_use]
    pub fn create_ublx_db_sql() -> String {
        format!(
            "{}{}{}{}{}{}",
            Self::CREATE_SNAPSHOT,
            Self::CREATE_SETTINGS,
            Self::CREATE_DELTA_LOG,
            Self::CREATE_PATH,
            Self::CREATE_LENS,
            Self::CREATE_LENS_PATH,
        )
    }
}

/// Statements for the ublx DB
pub struct UblxDbStatements;

impl UblxDbStatements {
    pub const INSERT_SNAPSHOT: &'static str = "INSERT OR REPLACE INTO snapshot (path, mtime_ns, size, hash, category, zahir_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

    /// Update category and `zahir_json` for an existing snapshot row (legacy; prefer [`Self::UPDATE_SNAPSHOT_ZAHIR_JSON_ONLY`] when category must stay unchanged).
    pub const UPDATE_SNAPSHOT_ZAHIR: &'static str =
        "UPDATE snapshot SET category = ?1, zahir_json = ?2 WHERE path = ?3";

    /// Update only `zahir_json` (category stays as set at insert / prior snapshot).
    pub const UPDATE_SNAPSHOT_ZAHIR_JSON_ONLY: &'static str =
        "UPDATE snapshot SET zahir_json = ?1 WHERE path = ?2";

    /// User rename on disk: repoint PK and refresh metadata without a full index (`hash` cleared until next full run).
    pub const UPDATE_SNAPSHOT_RENAME_IN_PLACE: &'static str = "UPDATE snapshot SET path = ?1, mtime_ns = ?2, size = ?3, hash = ?4, category = ?5, zahir_json = ?6 WHERE path = ?7";

    /// Set `hash` (32-byte blake3) for a row when duplicates scan fills missing hashes from disk.
    pub const UPDATE_SNAPSHOT_HASH_BY_PATH: &'static str =
        "UPDATE snapshot SET hash = ?1 WHERE path = ?2";

    pub const DELETE_SNAPSHOT_ROW: &'static str = "DELETE FROM snapshot WHERE path = ?1";

    pub const INSERT_SETTINGS: &'static str = "INSERT OR REPLACE INTO settings (id, num_threads, drive_type, parallel_walk, config_source) VALUES (1, ?1, ?2, ?3, ?4)";

    pub const INSERT_DELTA_LOG: &'static str = "INSERT INTO delta_log (created_ns, path, mtime_ns, size, hash, delta_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

    /// After a user rename on disk: keep historical `delta_log` rows aligned with the new path before appending removed/added rows.
    pub const UPDATE_DELTA_LOG_PATH: &'static str =
        "UPDATE delta_log SET path = ?1 WHERE path = ?2";

    pub const COPY_PREVIOUS_DELTA_LOG: &'static str = "INSERT INTO main.delta_log (created_ns, path, mtime_ns, size, hash, delta_type) SELECT created_ns, path, mtime_ns, size, hash, delta_type FROM old.delta_log";

    pub const ATTACH_OLD_DB: &'static str = "ATTACH DATABASE ?1 AS old";

    pub const DETACH_OLD_DB: &'static str = "DETACH DATABASE old";

    pub const SELECT_COUNT_DELTA_LOG_ROWS: &'static str =
        "SELECT COUNT(*) FROM old.sqlite_master WHERE type='table' AND name='delta_log'";

    /// Check if old DB has lens table (so we can copy `path/lens/lens_path`).
    pub const SELECT_LENS_TABLE_EXISTS: &'static str =
        "SELECT COUNT(*) FROM old.sqlite_master WHERE type='table' AND name='lens'";
    /// Copy path strings from old DB; new path ids are assigned in main.
    pub const COPY_PREVIOUS_PATH: &'static str =
        "INSERT INTO main.path(path) SELECT path FROM old.path";
    /// Copy lens names from old DB; new lens ids are assigned in main.
    pub const COPY_PREVIOUS_LENS: &'static str =
        "INSERT INTO main.lens(name) SELECT name FROM old.lens";
    /// Copy `lens_path` rows, mapping old `lens_id/path_id` to new ids by matching name/path.
    pub const COPY_PREVIOUS_LENS_PATH: &'static str = "INSERT INTO main.lens_path(lens_id, path_id, position) SELECT (SELECT id FROM main.lens WHERE name = (SELECT name FROM old.lens WHERE id = old.lens_path.lens_id)), (SELECT id FROM main.path WHERE path = (SELECT path FROM old.path WHERE id = old.lens_path.path_id)), position FROM old.lens_path WHERE (SELECT id FROM main.lens WHERE name = (SELECT name FROM old.lens WHERE id = old.lens_path.lens_id)) IS NOT NULL AND (SELECT id FROM main.path WHERE path = (SELECT path FROM old.path WHERE id = old.lens_path.path_id)) IS NOT NULL";

    /// (path, `zahir_json`) for paths that have non-empty `zahir_json`. Used for prior-zahir reuse.
    pub const SELECT_SNAPSHOT_PATH_ZAHIR_JSON: &'static str =
        "SELECT path, zahir_json FROM snapshot WHERE zahir_json IS NOT NULL AND zahir_json != ''";

    /// (path, category) for all snapshot rows. Used to preserve categories across Zahir enrichment.
    pub const SELECT_SNAPSHOT_PATH_CATEGORY: &'static str =
        "SELECT path, category FROM snapshot WHERE path IS NOT NULL";

    /// Distinct categories for TUI left bar.
    pub const SELECT_SNAPSHOT_CATEGORIES: &'static str = "SELECT DISTINCT category FROM snapshot WHERE category IS NOT NULL AND category != '' ORDER BY category";

    /// Distinct `created_ns` from `delta_log`, newest first (for Delta mode).
    pub const SELECT_DELTA_LOG_SNAPSHOT_TIMESTAMPS: &'static str =
        "SELECT DISTINCT created_ns FROM delta_log ORDER BY created_ns DESC";

    /// (`created_ns`, path) for a given `delta_type`, newest first then path.
    pub const SELECT_DELTA_LOG_ROWS_BY_TYPE: &'static str = "SELECT created_ns, path FROM delta_log WHERE delta_type = ?1 ORDER BY created_ns DESC, path";

    /// (path, category, size) for TUI list; `zahir_json` loaded on demand for selected row.
    pub const SELECT_SNAPSHOT_ROWS_FOR_TUI_BY_CATEGORY: &'static str =
        "SELECT path, category, size FROM snapshot WHERE category = ?1 ORDER BY path";

    /// (path, category, size) for TUI list; `zahir_json` loaded on demand for selected row.
    pub const SELECT_SNAPSHOT_ROWS_FOR_TUI_ALL: &'static str =
        "SELECT path, category, size FROM snapshot ORDER BY category, path";

    /// Single pass for TUI cold start: prior Nefax columns + category (no `zahir_json`). Order matches list query.
    pub const SELECT_SNAPSHOT_TUI_START: &'static str =
        "SELECT path, mtime_ns, size, hash, category FROM snapshot ORDER BY category, path";

    /// `zahir_json` for a single path (for right-pane on-demand load).
    pub const SELECT_SNAPSHOT_ZAHIR_JSON_BY_PATH: &'static str =
        "SELECT zahir_json FROM snapshot WHERE path = ?1";

    /// `mtime_ns` for a single path (for viewer footer last-modified).
    pub const SELECT_SNAPSHOT_MTIME_BY_PATH: &'static str =
        "SELECT mtime_ns FROM snapshot WHERE path = ?1";

    /// (path, `mtime_ns`) for all snapshot rows (used by Mod sort in middle pane).
    pub const SELECT_SNAPSHOT_PATH_MTIME_ALL: &'static str = "SELECT path, mtime_ns FROM snapshot";

    /// (path, size, hash) for non-directory rows; used for duplicate detection (by hash or content).
    pub const SELECT_SNAPSHOT_PATH_SIZE_HASH: &'static str =
        "SELECT path, size, hash FROM snapshot WHERE category IS NULL OR category != 'Directory'";

    /// Lens table: list lens names for TUI.
    pub const SELECT_LENS_NAMES: &'static str = "SELECT name FROM lens ORDER BY id";

    /// Lens id by name (for loading paths).
    pub const SELECT_LENS_ID_BY_NAME: &'static str = "SELECT id FROM lens WHERE name = ?1";

    /// Path id by path string (for `lens_path`).
    pub const SELECT_PATH_ID_BY_PATH: &'static str = "SELECT id FROM path WHERE path = ?1";

    /// Insert path, return id (use INSERT OR IGNORE then SELECT id).
    pub const INSERT_PATH: &'static str = "INSERT OR IGNORE INTO path (path) VALUES (?1)";

    /// Rename a normalized path string (lens `path` table); `lens_path` rows follow `path_id`.
    pub const UPDATE_PATH_STRING: &'static str = "UPDATE path SET path = ?1 WHERE path = ?2";

    /// Remove a path row (`lens_path` rows CASCADE). Call after deleting/moving the file on disk.
    pub const DELETE_PATH_ROW: &'static str = "DELETE FROM path WHERE path = ?1";

    pub const INSERT_LENS: &'static str = "INSERT INTO lens (name) VALUES (?1)";

    /// (`path_id`, position) for a lens, ordered by position. Join with path to get path string.
    pub const SELECT_LENS_PATH_IDS: &'static str =
        "SELECT path_id, position FROM lens_path WHERE lens_id = ?1 ORDER BY position";

    pub const INSERT_LENS_PATH: &'static str =
        "INSERT OR REPLACE INTO lens_path (lens_id, path_id, position) VALUES (?1, ?2, ?3)";

    /// Remove one path from a lens (by lens name and path string).
    pub const DELETE_LENS_PATH_ROW: &'static str = "DELETE FROM lens_path WHERE lens_id = (SELECT id FROM lens WHERE name = ?1) AND path_id = (SELECT id FROM path WHERE path = ?2)";
    /// Rename a lens.
    pub const UPDATE_LENS_NAME: &'static str = "UPDATE lens SET name = ?2 WHERE name = ?1";
    /// Delete a lens (`lens_path` rows removed by FK CASCADE).
    pub const DELETE_LENS: &'static str = "DELETE FROM lens WHERE name = ?1";

    /// (path, category, size) for TUI list for a lens; joins `lens_path`, path, and snapshot for category/size.
    pub const SELECT_LENS_ROWS_FOR_TUI: &'static str = "
        SELECT p.path, COALESCE(s.category, ''), COALESCE(s.size, 0)
        FROM lens_path lp
        JOIN path p ON lp.path_id = p.id
        LEFT JOIN snapshot s ON s.path = p.path
        WHERE lp.lens_id = ?1
        ORDER BY lp.position";

    #[must_use]
    pub fn create_query_for_nefax_from_db(table_name: &str) -> String {
        format!("SELECT path, mtime_ns, size, hash FROM {table_name}")
    }

    #[must_use]
    pub fn create_query_for_settings_from_db() -> String {
        "SELECT num_threads, drive_type, parallel_walk, config_source FROM settings WHERE id = 1"
            .to_string()
    }
}

/// Delta type for the `delta_log` table. Order (0 = Added, 1 = Mod, 2 = Removed) is used for TUI category index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeltaType {
    Added,
    Mod,
    Removed,
}

/// Number of delta categories (Added, Mod, Removed). Use for TUI category list length.
pub const DELTA_CATEGORY_COUNT: usize = 3;

impl DeltaType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            DeltaType::Added => "added",
            DeltaType::Mod => "mod",
            DeltaType::Removed => "removed",
        }
    }

    /// Index for TUI left-pane category (0 = Added, 1 = Mod, 2 = Removed).
    #[allow(dead_code)]
    #[must_use]
    pub const fn as_index(self) -> usize {
        match self {
            DeltaType::Added => 0,
            DeltaType::Mod => 1,
            DeltaType::Removed => 2,
        }
    }

    /// Delta type for the given TUI category index. Out-of-range maps to Removed.
    #[must_use]
    pub const fn from_index(idx: usize) -> Self {
        match idx {
            0 => DeltaType::Added,
            1 => DeltaType::Mod,
            _ => DeltaType::Removed,
        }
    }

    pub fn iter() -> impl Iterator<Item = DeltaType> {
        [DeltaType::Added, DeltaType::Mod, DeltaType::Removed].into_iter()
    }
}

/// Category for ublx db: ublx-defined variants plus all [`ZahirFT`] (zahirscan) via [`UblxDbCategory::Zahir`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UblxDbCategory {
    UblxLog,
    Git,
    // Hidden,
    Directory,
    File,
    /// All zahirscan file types; use [`ZahirFT::as_metadata_name`] for the display string.
    Zahir(ZahirFT),
}

impl UblxDbCategory {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            UblxDbCategory::UblxLog => "UBLX Log",
            UblxDbCategory::Git => "Git",
            // UblxDbCategory::Hidden => "Hidden",
            UblxDbCategory::Directory => "Directory",
            UblxDbCategory::File => "File",
            UblxDbCategory::Zahir(ft) => ft.as_metadata_name(),
        }
    }

    /// Parse a snapshot row `category` column (strings written by [`UblxDbCategory::get_category_for_path`]).
    #[must_use]
    pub fn from_snapshot_category(s: &str) -> Self {
        let t = s.trim();
        if t.is_empty() {
            return Self::File;
        }
        if t == Self::UblxLog.as_str() {
            return Self::UblxLog;
        }
        if t == Self::Git.as_str() {
            return Self::Git;
        }
        if t == Self::Directory.as_str() {
            return Self::Directory;
        }
        if t == Self::File.as_str() {
            return Self::File;
        }
        if let Some(ft) = file_type_from_metadata_name(t) {
            return Self::Zahir(ft);
        }
        Self::File
    }

    /// Get the category for a given path.
    #[must_use]
    pub fn get_category_for_path(
        path_ref: &Path,
        ublx_paths: Option<&UblxPaths>,
        zahir_file_type: Option<&str>,
    ) -> String {
        // Check for ublx.log
        if ublx_paths.is_some_and(|p| p.log_path() == path_ref) {
            return UblxDbCategory::UblxLog.as_str().to_string();
        }
        // Check for .git
        if Self::is_git_path(path_ref) {
            return UblxDbCategory::Git.as_str().to_string();
        }
        // Check for hidden files
        // if Self::is_hidden_path(path_ref) {
        //     return UblxDbCategory::Hidden.as_str().to_string();
        // }
        if Self::is_directory_path(path_ref) {
            if Self::is_zarr_store_directory(path_ref) {
                return Self::get_zahir_file_type_or_fallback(
                    zahir_file_type,
                    UblxDbCategory::Zahir(ZahirFT::Zarr),
                );
            }
            // Other directories before path-hint (extension/linguist can misclassify e.g. `Vol.1`).
            return UblxDbCategory::Directory.as_str().to_string();
        }
        Self::get_zahir_file_type_or_fallback(zahir_file_type, UblxDbCategory::File)
    }

    /// Fallback when zahir gives nothing usable (`File` for files, `Zahir(Zarr)` for a `.zarr` folder).
    fn get_zahir_file_type_or_fallback(
        zahir_file_type: Option<&str>,
        no_usable_hint: UblxDbCategory,
    ) -> String {
        let fallback = no_usable_hint.as_str().to_string();
        let s = match zahir_file_type {
            None => return fallback,
            Some(s) => s.trim(),
        };
        if s.is_empty() || s.eq_ignore_ascii_case("Unknown") {
            return fallback;
        }
        if let Some(ft) = file_type_from_metadata_name(s) {
            return UblxDbCategory::Zahir(ft).as_str().to_string();
        }
        // Non-metadata label (future zahir strings or legacy rows): keep as stored.
        s.to_string()
    }

    #[inline]
    #[allow(dead_code)]
    fn is_hidden_path(path_ref: &Path) -> bool {
        path_ref
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
    }

    #[inline]
    fn is_git_path(path_ref: &Path) -> bool {
        path_ref.to_string_lossy().contains(".git")
    }

    #[inline]
    fn is_directory_path(path_ref: &Path) -> bool {
        fs::metadata(path_ref).is_ok_and(|m| m.is_dir())
    }

    /// Last path segment is `*.zarr` (Zarr store folder on disk).
    #[inline]
    fn is_zarr_store_directory(path_ref: &Path) -> bool {
        path_ref
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".zarr"))
    }
}
