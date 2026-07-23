//! Per-root [`UblxPaths`] — DB / TOML / WAL / exclude paths.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::dirs::{cache_dir, db_dir, global_config_toml, last_applied_config_path};
use super::names::{UBLX_NAMES, path_to_hex, sanitize_name_for_fs};

/// Paths for the index DB and related files under an indexed `dir_to_ublx_abs`. Filenames use [`INDEX_DB_FILE_EXT`] and related suffixes (`_tmp`, `-wal`, `-shm`).
#[derive(Clone, Debug)]
pub struct UblxPaths {
    pub dir_to_ublx_abs: PathBuf,
}

impl UblxPaths {
    #[must_use]
    pub fn new(dir_to_ublx: &Path) -> Self {
        Self {
            dir_to_ublx_abs: dir_to_ublx.to_path_buf(),
        }
    }

    /// Filename stem (no extension) for the index DB: sanitized dir name + path hash.
    fn db_stem(&self) -> String {
        let dir_name = self
            .dir_to_ublx_abs
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root");
        let safe_name = sanitize_name_for_fs(dir_name);
        let hash = path_to_hex(&self.dir_to_ublx_abs);
        format!("{safe_name}_{hash}")
    }

    /// Full filename for the index DB under [`Self::db_dir`] (stem + [`INDEX_DB_FILE_EXT`]).
    #[must_use]
    fn db_filename(&self) -> String {
        format!("{}{}", self.db_stem(), UBLX_NAMES.index_db_file_ext)
    }

    #[must_use]
    pub fn db_dir(&self) -> Option<PathBuf> {
        db_dir()
    }

    /// Ensure the cache db folder exists.
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when creating the db directory fails.
    pub fn ensure_db_dir(&self) -> Result<PathBuf> {
        let dir = self
            .db_dir()
            .ok_or_else(|| anyhow::anyhow!("could not resolve user cache directory"))?;
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    #[must_use]
    pub fn log_path(&self) -> PathBuf {
        self.dir_to_ublx_abs
            .join(format!("{}.log", UBLX_NAMES.pkg_name))
    }

    /// Hidden config path: `dir_to_ublx_abs/.ublx.toml`.
    #[must_use]
    pub fn hidden_toml(&self) -> PathBuf {
        self.dir_to_ublx_abs
            .join(UBLX_NAMES.local_config_hidden_toml)
    }

    /// Visible config path: `dir_to_ublx_abs/ublx.toml`.
    #[must_use]
    pub fn visible_toml(&self) -> PathBuf {
        self.dir_to_ublx_abs
            .join(UBLX_NAMES.local_config_visible_toml)
    }

    /// True if `path` (relative to `dir_to_ublx_abs`) is the hidden or visible ublx config file.
    #[must_use]
    pub fn is_config_file(&self, path: &Path) -> bool {
        let Some(name) = path.file_name() else {
            return false;
        };
        name == OsStr::new(UBLX_NAMES.local_config_visible_toml)
            || name == OsStr::new(UBLX_NAMES.local_config_hidden_toml)
    }

    /// Path to the config file to use: checks for `dir_to_ublx_abs/.ublx.toml` then `dir_to_ublx_abs/ublx.toml`; returns the first that exists, or `None`.
    #[must_use]
    pub fn toml_path(&self) -> Option<PathBuf> {
        let hidden = self.hidden_toml();
        let visible = self.visible_toml();
        if hidden.exists() {
            Some(hidden)
        } else if visible.exists() {
            Some(visible)
        } else {
            None
        }
    }

    /// Path used when creating or updating local config: existing hidden or visible file if present, otherwise
    /// [`Self::hidden_toml`] (same default as other local-config writers in this crate).
    #[must_use]
    pub fn local_config_path_for_write(&self) -> PathBuf {
        self.toml_path().unwrap_or_else(|| self.hidden_toml())
    }

    /// Main DB file under `cache_dir()` / [`UBLX_NAMES.pkg_name_plural`] (basename + [`INDEX_DB_FILE_EXT`]). `SQLite` creates it if missing.
    #[must_use]
    pub fn db(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(self.db_filename())
    }

    /// Nefaxer index file (e.g. `dir_to_ublx_abs/.nefaxer`). When present, used as prior snapshot before ublx snapshot.
    #[must_use]
    pub fn nefax_db(&self) -> PathBuf {
        self.dir_to_ublx_abs.join(UBLX_NAMES.nefax_db)
    }

    /// Temp file (write-then-rename to [`Self::db`]). Same stem as DB with `_tmp` before [`INDEX_DB_FILE_EXT`].
    #[must_use]
    pub fn tmp(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}_tmp{}",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// WAL file for [`Self::tmp`] when snapshot build uses `journal_mode=WAL`.
    #[must_use]
    pub fn tmp_wal(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}_tmp{}-wal",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// Shared-memory file for [`Self::tmp`] in WAL mode.
    #[must_use]
    pub fn tmp_shm(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}_tmp{}-shm",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// `SQLite` WAL file for [`Self::db`] when WAL mode is on.
    #[must_use]
    pub fn wal(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}{}-wal",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// `SQLite` shared-memory file for [`Self::db`] in WAL mode.
    #[must_use]
    pub fn shm(&self) -> PathBuf {
        self.db_dir()
            .unwrap_or_else(|| self.dir_to_ublx_abs.clone())
            .join(format!(
                "{}{}-shm",
                self.db_stem(),
                UBLX_NAMES.index_db_file_ext
            ))
    }

    /// Paths to exclude from indexing (nefax + local config). DB files under `ubli/` use [`INDEX_DB_FILE_EXT`] and are not listed here (nefax matches path components).
    /// Local `ublx.toml` / `.ublx.toml` are edited from the Settings tab, not listed as a snapshot category.
    /// [`UblxNames::zahir_export_dir_name`] (flat Zahir JSON export) and [`UblxNames::lens_export_dir_name`] (lens Markdown export) are excluded so re-indexing does not ingest them.
    #[must_use]
    pub fn exclude(&self) -> Vec<String> {
        vec![
            UBLX_NAMES.nefax_db.to_string(),
            UBLX_NAMES.local_config_visible_toml.to_string(),
            UBLX_NAMES.local_config_hidden_toml.to_string(),
            UBLX_NAMES.zahir_export_dir_name.to_string(),
            // UBLX_NAMES.lens_export_dir_name.to_string(),
        ]
    }

    /// Remove tmp, WAL, and SHM files if they exist. No error if any are missing.
    /// Close the DB connection before calling if you use WAL mode.
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when removing an existing auxiliary file fails (e.g. I/O permission denied).
    pub fn remove_aux_files(&self) -> Result<(), anyhow::Error> {
        for p in [
            self.tmp(),
            self.tmp_wal(),
            self.tmp_shm(),
            self.wal(),
            self.shm(),
        ] {
            if p.exists() {
                fs::remove_file(&p)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn global_config(&self) -> Option<PathBuf> {
        global_config_toml()
    }

    /// User cache dir (`~/.local/share/ublx` or Windows equivalent). Used for last-applied config and future hot-reload fallback.
    #[allow(dead_code)]
    #[must_use]
    pub fn cache_dir(&self) -> Option<PathBuf> {
        cache_dir()
    }

    #[must_use]
    pub fn last_applied_config_path(&self) -> Option<PathBuf> {
        last_applied_config_path(&self.dir_to_ublx_abs)
    }
}

#[must_use]
pub fn get_log_path(dir_to_ublx: &Path) -> PathBuf {
    UblxPaths::new(dir_to_ublx).log_path()
}

#[must_use]
/// Normalize a path string for policy matching (e.g. `photos/vacation` → `photos/vacation`)
pub fn normalize_rel_path_for_policy(s: &str) -> String {
    let s = s.replace('\\', "/");
    let s = s.trim_start_matches("./");
    s.trim_end_matches('/').to_string()
}

/// True if `rel` (relative path) is under or equal to `prefix` (e.g. `photos/vacation` is under `photos`).
#[must_use]
pub fn path_is_under_or_equal(rel: &str, prefix: &str) -> bool {
    rel == prefix || (rel.starts_with(prefix) && rel.as_bytes().get(prefix.len()) == Some(&b'/'))
}
