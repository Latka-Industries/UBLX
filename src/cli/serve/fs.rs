//! File-system / enhance mutations for Space + bulk menus (`POST /fs/…`).

use std::path::PathBuf;

use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::config::{EnhancePolicy, UblxOpts, UblxPaths, write_local_enhance_policy};
use crate::integrations::ZahirRC;
use crate::modules::{enhancer, file_ops};

use super::content::paths::require_rel_path;
use super::error::ApiError;
use super::state::{AppState, mutation_paths, reopen_catalog};

#[derive(Debug, Deserialize)]
pub(super) struct RenameBody {
    path: String,
    new_name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct BulkRenameItem {
    path: String,
    new_name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct BulkRenameBody {
    renames: Vec<BulkRenameItem>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PathsBody {
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct EnhancePolicyBody {
    path: String,
    /// `"auto"` or `"manual"`.
    policy: String,
}

#[derive(Debug, Serialize)]
struct RenameOut {
    path: String,
}

#[derive(Debug, Serialize)]
struct BulkOpFail {
    path: String,
    error: String,
}

#[derive(Debug, Serialize)]
struct BulkRenameOut {
    renamed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed: Option<BulkOpFail>,
}

#[derive(Debug, Serialize)]
struct BulkDeleteOut {
    deleted: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed: Option<BulkOpFail>,
}

#[derive(Debug, Serialize)]
struct EnhanceOut {
    enhanced: usize,
    failed: usize,
}

#[derive(Debug, Serialize)]
struct PolicyOut {
    policy: String,
}

pub(super) async fn post_rename(
    State(state): State<AppState>,
    Json(body): Json<RenameBody>,
) -> Result<impl IntoResponse, ApiError> {
    let rel = require_rel_path(&body.path)?;
    let (dir, db) = mutation_paths(&state)?;
    let new_path = file_ops::rename_entry_under_root(&dir, &db, &rel, &body.new_name)?;
    reopen_catalog(&state)?;
    Ok(Json(RenameOut { path: new_path }))
}

pub(super) async fn post_bulk_rename(
    State(state): State<AppState>,
    Json(body): Json<BulkRenameBody>,
) -> Result<impl IntoResponse, ApiError> {
    let (dir, db) = mutation_paths(&state)?;
    let mut renamed = 0usize;
    let mut failed = None;
    for item in body.renames {
        let rel = match require_rel_path(&item.path) {
            Ok(p) => p,
            Err(_) => {
                failed = Some(BulkOpFail {
                    path: item.path,
                    error: "invalid entry path".into(),
                });
                break;
            }
        };
        match file_ops::rename_entry_under_root(&dir, &db, &rel, &item.new_name) {
            Ok(_) => renamed += 1,
            Err(e) => {
                failed = Some(BulkOpFail {
                    path: rel,
                    error: e.to_string(),
                });
                break;
            }
        }
    }
    if renamed > 0 {
        reopen_catalog(&state)?;
    }
    Ok(Json(BulkRenameOut { renamed, failed }))
}

pub(super) async fn post_delete(
    State(state): State<AppState>,
    Json(body): Json<PathsBody>,
) -> Result<impl IntoResponse, ApiError> {
    if body.paths.is_empty() {
        return Err(ApiError::bad_request("paths must not be empty"));
    }
    let (dir, db) = mutation_paths(&state)?;
    let mut deleted = 0usize;
    let mut failed = None;
    for raw in body.paths {
        let rel = match require_rel_path(&raw) {
            Ok(p) => p,
            Err(_) => {
                failed = Some(BulkOpFail {
                    path: raw,
                    error: "invalid entry path".into(),
                });
                break;
            }
        };
        match file_ops::delete_entry_under_root(&dir, &db, &rel) {
            Ok(()) => deleted += 1,
            Err(e) => {
                failed = Some(BulkOpFail {
                    path: rel,
                    error: e.to_string(),
                });
                break;
            }
        }
    }
    if deleted > 0 {
        reopen_catalog(&state)?;
    }
    Ok(Json(BulkDeleteOut { deleted, failed }))
}

pub(super) async fn post_enhance(
    State(state): State<AppState>,
    Json(body): Json<PathsBody>,
) -> Result<impl IntoResponse, ApiError> {
    if body.paths.is_empty() {
        return Err(ApiError::bad_request("paths must not be empty"));
    }
    let (dir, db) = mutation_paths(&state)?;
    let opts = UblxOpts::for_zahir_only(4, ZahirRC::new());
    let mut enhanced = 0usize;
    let mut failed = 0usize;
    for raw in body.paths {
        let Ok(rel) = require_rel_path(&raw) else {
            failed += 1;
            continue;
        };
        let abs = PathBuf::from(&dir).join(&rel);
        if abs.is_dir() {
            failed += 1;
            continue;
        }
        match enhancer::enhance_single_path(&dir, &db, &rel, &opts) {
            Ok(()) => enhanced += 1,
            Err(e) => {
                warn!("serve enhance failed for {rel}: {e}");
                failed += 1;
            }
        }
    }
    if enhanced > 0 {
        reopen_catalog(&state)?;
    }
    Ok(Json(EnhanceOut { enhanced, failed }))
}

pub(super) async fn post_enhance_policy(
    State(state): State<AppState>,
    Json(body): Json<EnhancePolicyBody>,
) -> Result<impl IntoResponse, ApiError> {
    let rel = require_rel_path(&body.path)?;
    let policy = match body.policy.trim().to_ascii_lowercase().as_str() {
        "auto" | "always" => EnhancePolicy::Auto,
        "manual" | "never" => EnhancePolicy::Manual,
        _ => {
            return Err(ApiError::bad_request(
                "policy must be \"auto\" or \"manual\"",
            ));
        }
    };
    let dir = super::state::current_dir(&state)?;
    let paths = UblxPaths::new(&dir);
    write_local_enhance_policy(&paths, &rel, policy);
    let label = match policy {
        EnhancePolicy::Auto => "auto",
        EnhancePolicy::Manual => "manual",
    };
    Ok(Json(PolicyOut {
        policy: label.into(),
    }))
}
