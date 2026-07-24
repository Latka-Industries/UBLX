//! Resolve indexed DIR → `.ublx` path and open a read connection for headless subcommands.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use rusqlite::Connection;

use crate::config::UblxPaths;
use crate::engine::db_ops::{
    SnapshotReaderPreference, UblxDbSchema, open_for_snapshot_tui_read, snapshot_reader_path_with,
};
use crate::utils;

/// Paths for a catalog under an indexed directory (before opening).
#[derive(Debug, Clone)]
pub struct CatalogPaths {
    pub dir: PathBuf,
    pub paths: UblxPaths,
    /// Expected final DB path from [`UblxPaths::db`] (may not exist yet).
    pub db_path: PathBuf,
}

/// Opened catalog for read-only headless use (`query`, `doctor`, later `serve`).
pub struct CatalogHandle {
    pub paths: CatalogPaths,
    /// File actually opened (`.ublx` or `.ublx_tmp` via [`snapshot_reader_path_with`]).
    pub read_path: PathBuf,
    pub conn: Connection,
}

/// Canonicalize `dir` the same way as the TUI entry path.
///
/// # Errors
///
/// Returns `Err` when the path is missing, not a directory, or cannot be canonicalized.
pub fn resolve_catalog_dir(dir: &Path) -> Result<PathBuf, String> {
    utils::try_validate_dir(dir)
}

/// Resolve expected catalog paths for `dir` without creating the DB.
///
/// # Errors
///
/// Returns `Err` when directory validation fails.
pub fn resolve_catalog_paths(dir: &Path) -> Result<CatalogPaths, anyhow::Error> {
    let dir = resolve_catalog_dir(dir).map_err(|e| anyhow::anyhow!("{e}"))?;
    let paths = UblxPaths::new(&dir);
    let db_path = paths.db();
    Ok(CatalogPaths {
        dir,
        paths,
        db_path,
    })
}

/// True when `.ublx` or a readable `.ublx_tmp` exists for this catalog root.
#[must_use]
pub fn catalog_db_present(catalog_paths: &CatalogPaths) -> bool {
    snapshot_reader_path_with(&catalog_paths.db_path, SnapshotReaderPreference::PreferUblx)
        .is_some()
}

/// Resolve catalog paths and open a read connection (same tuning as TUI snapshot reads).
///
/// Does **not** create the DB — fails if neither `.ublx` nor `.ublx_tmp` exists.
///
/// # Errors
///
/// Returns `Err` when the directory is invalid, no catalog file exists, or `SQLite` open fails.
pub fn open_catalog_for_read(dir: &Path) -> Result<CatalogHandle, anyhow::Error> {
    let catalog_paths = resolve_catalog_paths(dir)?;
    let Some(read_path) =
        snapshot_reader_path_with(&catalog_paths.db_path, SnapshotReaderPreference::PreferUblx)
    else {
        bail!(
            "no catalog DB for {} (expected {}); run `ublx` or `ublx -s` in that directory first",
            catalog_paths.dir.display(),
            catalog_paths.db_path.display()
        );
    };
    let conn = open_for_snapshot_tui_read(&read_path)
        .with_context(|| format!("failed to open catalog {}", read_path.display()))?;
    Ok(CatalogHandle {
        paths: catalog_paths,
        read_path,
        conn,
    })
}

/// In-memory empty schema so `ublx serve` can bind before the first on-disk snapshot lands.
///
/// `read_path` is the expected final `.ublx` path (not opened yet). Callers must kick a snapshot
/// and reopen via [`open_catalog_for_read`] when it finishes.
///
/// # Errors
///
/// Returns `Err` when the in-memory DB cannot be created or schema apply fails.
pub fn open_placeholder_catalog(
    catalog_paths: CatalogPaths,
) -> Result<CatalogHandle, anyhow::Error> {
    let conn =
        Connection::open_in_memory().context("failed to open in-memory placeholder catalog")?;
    conn.execute_batch(&UblxDbSchema::create_ublx_db_sql())
        .context("failed to apply schema to placeholder catalog")?;
    let read_path = catalog_paths.db_path.clone();
    Ok(CatalogHandle {
        paths: catalog_paths,
        read_path,
        conn,
    })
}

/// Heuristic: a snapshot pipeline is likely writing when `.ublx_tmp` exists **and** its WAL/SHM
/// sidecars are present (live WAL mode during build). A lone leftover `.ublx_tmp` after a crash
/// does **not** count — doctor/`--fix` can still clean that.
#[must_use]
pub fn snapshot_likely_in_progress(catalog_paths: &CatalogPaths) -> bool {
    let p = &catalog_paths.paths;
    p.tmp().exists() && (p.tmp_wal().exists() || p.tmp_shm().exists())
}
