//! On-disk exports for Command Mode (`POST /export/…`).

use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use log::info;
use serde::Serialize;

use crate::config::UBLX_NAMES;
use crate::engine::db_ops::{export_lenses_markdown_flat, export_zahir_json_flat};

use super::error::ApiError;
use super::state::{AppState, ensure_snapshot_idle, mutation_paths};

#[derive(Debug, Serialize)]
struct ExportOut {
    count: usize,
    dir: String,
}

pub(super) async fn post_export_zahir(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_snapshot_idle(&state)?;
    let (dir, db) = mutation_paths(&state)?;
    let count = export_zahir_json_flat(&dir, &db).map_err(ApiError::from)?;
    let out_dir = dir.join(UBLX_NAMES.zahir_export_dir_name);
    info!(
        "serve zahir export: dir={} count={count}",
        out_dir.display()
    );
    Ok(Json(ExportOut {
        count,
        dir: out_dir.display().to_string(),
    }))
}

pub(super) async fn post_export_lenses(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_snapshot_idle(&state)?;
    let (dir, db) = mutation_paths(&state)?;
    let count = export_lenses_markdown_flat(&dir, &db).map_err(ApiError::from)?;
    let out_dir = dir.join(UBLX_NAMES.lens_export_dir_name);
    info!("serve lens export: dir={} count={count}", out_dir.display());
    Ok(Json(ExportOut {
        count,
        dir: out_dir.display().to_string(),
    }))
}
