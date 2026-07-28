//! Small path / time helpers used by catalog resolve, `db_ops`, and headless open.

use std::path::{Path, PathBuf};

/// Expand a leading `~/` using `HOME` so `cargo run -- ~/src/proj` works (the shell often does not expand `~` in argv).
#[must_use]
pub fn expand_home_dir_arg(path: &Path) -> PathBuf {
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    let Some(rest) = s.strip_prefix("~/") else {
        return path.to_path_buf();
    };
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    #[cfg(windows)]
    {
        if let Ok(user) = std::env::var("USERPROFILE") {
            return PathBuf::from(user).join(rest);
        }
    }
    path.to_path_buf()
}

/// Validate that a path is a directory and return the canonicalized path.
/// Symlinks are resolved (e.g. `~/Dropbox` → `~/Library/CloudStorage/...` on macOS).
///
/// # Errors
///
/// Returns `Err` with a message if the path does not exist, is not a directory, or cannot be canonicalized.
pub fn try_validate_dir(path: &Path) -> Result<PathBuf, String> {
    let path = expand_home_dir_arg(path);
    if !path.exists() {
        return Err(format!("no such file or directory: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("not a directory: {}", path.display()));
    }
    path.canonicalize()
        .map_err(|e| format!("cannot canonicalize '{}': {e}", path.display()))
}

/// Canonicalized indexed root, or the path unchanged when canonicalization fails.
#[must_use]
pub fn canonicalize_dir_to_ublx(dir_to_ublx: &Path) -> PathBuf {
    dir_to_ublx
        .canonicalize()
        .unwrap_or_else(|_| dir_to_ublx.to_path_buf())
}

/// Normalize a snapshot `path` column so it matches nefaxer's relative path strings (`rel_str` / map keys).
///
/// Trims, strips a leading `./` or `.\`, then replaces `\` with `/`.
#[must_use]
pub fn normalize_snapshot_rel_path_str(path: &str) -> String {
    let mut s = path.trim();
    s = s.strip_prefix("./").unwrap_or(s);
    if let Some(rest) = s.strip_prefix(".\\") {
        s = rest;
    }
    s.replace('\\', "/")
}

/// [`PathBuf`] key for nefax-style maps, from a snapshot `path` column (see [`normalize_snapshot_rel_path_str`]).
#[must_use]
pub fn snapshot_rel_path_buf(path_str: &str) -> PathBuf {
    PathBuf::from(normalize_snapshot_rel_path_str(path_str))
}

/// Current Unix timestamp in nanoseconds (`delta_log.created_ns`).
#[must_use]
pub fn get_created_ns() -> i64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    i64::try_from(nanos).unwrap_or(i64::MAX)
}
