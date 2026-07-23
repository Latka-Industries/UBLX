//! `/roots` and `/roots/current`.

use std::path::Path;

use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::cli::catalog::open_catalog_for_read;
use crate::cli::doctor;
use crate::config::{all_indexed_roots_alphabetical, record_prior_root_selected};

use super::error::ApiError;
use super::state::{AppState, canonicalize_dir, current_dir, same_dir, with_inner};

#[derive(Debug, Serialize)]
struct RootRow {
    path: String,
    current: bool,
}

#[derive(Debug, Serialize)]
struct CurrentRoot {
    path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SwitchRootBody {
    /// Absolute or relative path to an indexed project root (same as TUI switch).
    dir: String,
}

pub(super) async fn get_roots(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let current_canon = canonicalize_dir(&current_dir(&state)?);
    let mut roots = all_indexed_roots_alphabetical();
    if !roots.iter().any(|p| same_dir(p, &current_canon)) {
        roots.push(current_canon.clone());
        roots.sort_by_key(|a| a.display().to_string());
    }
    let rows: Vec<RootRow> = roots
        .into_iter()
        .map(|p| RootRow {
            current: same_dir(&p, &current_canon),
            path: p.display().to_string(),
        })
        .collect();
    Ok(Json(rows))
}

pub(super) async fn get_current_root(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(CurrentRoot {
        path: current_dir(&state)?.display().to_string(),
    }))
}

pub(super) async fn put_current_root(
    State(state): State<AppState>,
    Json(body): Json<SwitchRootBody>,
) -> Result<impl IntoResponse, ApiError> {
    with_inner(&state, |inner| {
        if inner.snapshot.is_running() {
            return Err(ApiError::conflict(
                "snapshot in progress; wait for GET /snapshot to leave running before switching roots",
            ));
        }
        Ok(())
    })?;

    let handle = open_catalog_for_read(Path::new(&body.dir)).map_err(|e| {
        warn!("serve root switch failed for {}: {e}", body.dir);
        ApiError::from(e)
    })?;
    let new_dir = handle.paths.dir;
    let new_read_path = handle.read_path;
    let new_conn = handle.conn;
    with_inner(&state, |inner| {
        if same_dir(&inner.catalog.dir, &new_dir) {
            info!("serve root unchanged: {}", inner.catalog.dir.display());
        } else {
            let prev = inner.catalog.dir.display().to_string();
            inner.catalog.dir.clone_from(&new_dir);
            inner.catalog.read_path = new_read_path;
            inner.catalog.conn = new_conn;
            let _ = record_prior_root_selected(&new_dir);
            info!("serve root switched: {prev} -> {}", new_dir.display());
        }
        Ok(())
    })?;
    Ok(Json(CurrentRoot {
        path: new_dir.display().to_string(),
    }))
}

pub(super) async fn get_doctor(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let dir = current_dir(&state)?;
    let report = doctor::diagnose(&dir).map_err(ApiError::from)?;
    info!(
        "serve doctor: dir={} summary={:?}",
        report.dir, report.summary
    );
    Ok(Json(report))
}
