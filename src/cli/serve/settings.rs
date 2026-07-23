//! `/settings/{scope}` read/patch.

use axum::Json;
use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use log::info;

use crate::cli::settings_api::{self, SettingsPatch};

use super::error::ApiError;
use super::state::{AppState, current_dir};

pub(super) async fn get_settings(
    State(state): State<AppState>,
    AxumPath(scope): AxumPath<String>,
) -> Result<impl IntoResponse, ApiError> {
    let dir = current_dir(&state)?;
    let view = settings_api::get_settings_view(&dir, &scope).map_err(ApiError::bad_request)?;
    Ok(Json(view))
}

pub(super) async fn patch_settings_route(
    State(state): State<AppState>,
    AxumPath(scope): AxumPath<String>,
    Json(patch): Json<SettingsPatch>,
) -> Result<impl IntoResponse, ApiError> {
    let dir = current_dir(&state)?;
    let view = settings_api::patch_settings(&dir, &scope, &patch).map_err(ApiError::bad_request)?;
    info!(
        "serve settings patched: scope={scope} dir={}",
        dir.display()
    );
    Ok(Json(view))
}
