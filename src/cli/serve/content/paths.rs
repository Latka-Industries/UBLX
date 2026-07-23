//! Entry path validation and root-relative disk resolve.

use std::path::{Path, PathBuf};

use super::super::error::ApiError;
use super::super::state::canonicalize_dir;

/// Normalize and reject empty / `..` segments (path-traversal).
pub(in crate::cli::serve) fn require_rel_path(path: &str) -> Result<String, ApiError> {
    let path = normalize_entry_path(path);
    if path.is_empty() || path.split('/').any(|s| s == "..") {
        return Err(ApiError::bad_request("invalid entry path"));
    }
    Ok(path)
}

/// Join catalog-relative path under the current root; reject escapes outside the root.
pub(super) fn resolve_entry_disk_path(root: &Path, rel: &str) -> Result<PathBuf, ApiError> {
    let root = canonicalize_dir(root);
    let joined = root.join(rel);
    let canon = joined
        .canonicalize()
        .map_err(|e| ApiError::not_found(format!("file not found for {rel}: {e}")))?;
    if !canon.starts_with(&root) {
        return Err(ApiError::bad_request("path escapes project root"));
    }
    Ok(canon)
}

/// Axum may leave a leading slash on `{*path}`; catalog paths are relative without it.
fn normalize_entry_path(path: &str) -> String {
    path.trim_start_matches('/').to_string()
}
