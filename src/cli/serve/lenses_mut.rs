//! Lens create / rename / delete / path membership (`POST|PATCH|DELETE /lenses…`).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::modules::lenses;

use super::content::paths::require_rel_path;
use super::error::ApiError;
use super::state::{AppState, mutation_paths, reopen_catalog};

#[derive(Debug, Deserialize)]
pub(super) struct CreateLensBody {
    name: String,
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RenameLensBody {
    new_name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct LensPathsBody {
    paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CreateLensOut {
    name: String,
    added: usize,
}

#[derive(Debug, Serialize)]
struct RenameLensOut {
    name: String,
}

#[derive(Debug, Serialize)]
struct PathsMutOut {
    count: usize,
}

pub(super) async fn post_lens(
    State(state): State<AppState>,
    Json(body): Json<CreateLensBody>,
) -> Result<impl IntoResponse, ApiError> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("lens name is empty"));
    }
    let (_dir, db) = mutation_paths(&state)?;
    lenses::create_lens(&db, name)?;
    let mut added = 0usize;
    for raw in &body.paths {
        let rel = require_rel_path(raw)?;
        lenses::add_path_to_lens(&db, name, &rel)?;
        added += 1;
    }
    reopen_catalog(&state)?;
    Ok((
        StatusCode::CREATED,
        Json(CreateLensOut {
            name: name.to_string(),
            added,
        }),
    ))
}

pub(super) async fn patch_lens(
    State(state): State<AppState>,
    Path(old_name): Path<String>,
    Json(body): Json<RenameLensBody>,
) -> Result<impl IntoResponse, ApiError> {
    let new_name = body.new_name.trim();
    if new_name.is_empty() {
        return Err(ApiError::bad_request("lens name is empty"));
    }
    let (_dir, db) = mutation_paths(&state)?;
    lenses::rename_lens(&db, &old_name, new_name)?;
    reopen_catalog(&state)?;
    Ok(Json(RenameLensOut {
        name: new_name.to_string(),
    }))
}

pub(super) async fn delete_lens_route(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let (_dir, db) = mutation_paths(&state)?;
    lenses::delete_lens(&db, &name)?;
    reopen_catalog(&state)?;
    Ok(Json(serde_json::json!({})))
}

pub(super) async fn post_lens_paths(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<LensPathsBody>,
) -> Result<impl IntoResponse, ApiError> {
    if body.paths.is_empty() {
        return Err(ApiError::bad_request("paths must not be empty"));
    }
    let (_dir, db) = mutation_paths(&state)?;
    let mut count = 0usize;
    for raw in body.paths {
        let rel = require_rel_path(&raw)?;
        lenses::add_path_to_lens(&db, &name, &rel)?;
        count += 1;
    }
    reopen_catalog(&state)?;
    Ok(Json(PathsMutOut { count }))
}

pub(super) async fn delete_lens_paths(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<LensPathsBody>,
) -> Result<impl IntoResponse, ApiError> {
    if body.paths.is_empty() {
        return Err(ApiError::bad_request("paths must not be empty"));
    }
    let (_dir, db) = mutation_paths(&state)?;
    let mut count = 0usize;
    for raw in body.paths {
        let rel = require_rel_path(&raw)?;
        lenses::remove_path_from_lens(&db, &name, &rel)?;
        count += 1;
    }
    reopen_catalog(&state)?;
    Ok(Json(PathsMutOut { count }))
}
