//! Duplicate detection: group snapshot rows by stored DB hash only.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use super::consts::UblxDbStatements;
use super::{SnapshotPathSizeHash, load_snapshot_path_size_hash};

/// One group of duplicate paths (same content). Left panel shows one name per group.
#[derive(Clone, Debug)]
pub struct DuplicateGroup {
    pub paths: Vec<String>,
}

/// Which grouping strategy produced duplicate groups for the current load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuplicateGroupingMode {
    Hash,
    NameSize,
}

impl DuplicateGroup {
    /// Representative name for the left panel (one name per duplicate).
    pub fn representative_name(&self) -> &str {
        self.paths
            .iter()
            .min_by_key(|p| p.len())
            .map_or("", String::as_str)
    }
}

/// Group rows by 32-byte hash; keep only groups with more than one path.
fn group_by_hash(rows: &[SnapshotPathSizeHash]) -> Vec<DuplicateGroup> {
    let mut by_hash: HashMap<[u8; 32], Vec<String>> = HashMap::new();
    for (path, _size, hash_opt) in rows {
        let Some(blob) = hash_opt.as_ref() else {
            continue;
        };
        if blob.len() != 32 {
            continue;
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(blob);
        by_hash.entry(key).or_default().push(path.clone());
    }
    by_hash
        .into_values()
        .filter(|paths| paths.len() > 1)
        .map(|paths| DuplicateGroup { paths })
        .collect()
}

/// Group rows by `(basename, size)` for fast non-content duplicate hints.
fn group_by_name_size(rows: &[SnapshotPathSizeHash]) -> Vec<DuplicateGroup> {
    let mut by_name_size: HashMap<(String, u64), Vec<String>> = HashMap::new();
    for (path, size, _hash_opt) in rows {
        let base = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .map_or_else(|| path.clone(), ToString::to_string);
        by_name_size
            .entry((base, *size))
            .or_default()
            .push(path.clone());
    }
    by_name_size
        .into_values()
        .filter(|paths| paths.len() > 1)
        .map(|paths| DuplicateGroup { paths })
        .collect()
}

fn hash_file_blake3(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(*hasher.finalize().as_bytes())
}

/// For rows missing a 32-byte hash, read each file under `dir_to_ublx`, compute blake3, persist to DB, and refresh `rows`.
fn fill_missing_hashes_from_disk(
    db_path: &Path,
    dir_to_ublx: &Path,
    rows: &mut [SnapshotPathSizeHash],
) -> Result<(), anyhow::Error> {
    let conn = Connection::open(db_path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    for (path, _size, hash_opt) in rows.iter_mut() {
        if hash_opt.as_ref().is_some_and(|v| v.len() == 32) {
            continue;
        }
        let abs = dir_to_ublx.join(path.as_str());
        if !abs.is_file() {
            continue;
        }
        let h = match hash_file_blake3(&abs) {
            Ok(h) => h,
            Err(e) => {
                log::debug!("duplicates: skip hash for {}: {e}", abs.display());
                continue;
            }
        };
        let blob: Vec<u8> = h.to_vec();
        conn.execute(
            UblxDbStatements::UPDATE_SNAPSHOT_HASH_BY_PATH,
            rusqlite::params![blob.clone(), path.as_str()],
        )?;
        *hash_opt = Some(blob);
    }
    Ok(())
}

fn any_valid_hash(rows: &[SnapshotPathSizeHash]) -> bool {
    rows.iter()
        .any(|(_, _, h)| h.as_ref().is_some_and(|v| v.len() == 32))
}

/// Load duplicate groups from the snapshot DB.
///
/// - If any row has a valid 32-byte hash, groups by hash ([`DuplicateGroupingMode::Hash`]).
/// - Else, when `config_wants_hash` is true (merged `ublx.toml` **`hash`** → nefax `with_hash`),
///   computes blake3 for each indexed file, writes `hash` into the DB, then groups by hash.
/// - Otherwise groups by `(basename, size)` ([`DuplicateGroupingMode::NameSize`]).
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` open/query errors or hash I/O failures bubbled from updates.
pub fn load_duplicate_groups(
    db_path: &Path,
    dir_to_ublx: &Path,
    config_wants_hash: bool,
) -> Result<(Vec<DuplicateGroup>, DuplicateGroupingMode), anyhow::Error> {
    let mut rows = load_snapshot_path_size_hash(db_path)?;
    if any_valid_hash(&rows) {
        return Ok((group_by_hash(&rows), DuplicateGroupingMode::Hash));
    }

    if config_wants_hash {
        fill_missing_hashes_from_disk(db_path, dir_to_ublx, &mut rows)?;
        if any_valid_hash(&rows) {
            return Ok((group_by_hash(&rows), DuplicateGroupingMode::Hash));
        }
    }

    Ok((group_by_name_size(&rows), DuplicateGroupingMode::NameSize))
}
