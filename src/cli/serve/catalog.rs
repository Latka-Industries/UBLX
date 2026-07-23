//! Catalog list/detail routes: entries, categories, delta, lenses, duplicates.

use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use axum::response::{IntoResponse, Response};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::cli::catalog_read::{
    EntryListFilter, EntryRow, entry_detail, is_not_found, list_categories, list_delta,
    list_duplicates, list_entries, list_lens_entries, list_lens_names,
};
use crate::cli::settings_api;
use crate::handlers::viewing::sectioned_preview_from_zahir;
use crate::render::kv_tables::{SectionView, parse_json_to_views};

use super::content::paths::require_rel_path;
use super::error::ApiError;
use super::state::{AppState, current_dir, with_db, with_inner};

#[derive(Debug, Deserialize)]
pub(super) struct EntriesQuery {
    category: Option<String>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    contains: Option<String>,
}

impl EntriesQuery {
    fn filter(&self) -> EntryListFilter<'_> {
        EntryListFilter::new(
            self.category.as_deref(),
            self.min_size,
            self.max_size,
            self.contains.as_deref(),
        )
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct EntryQuery {
    #[serde(default, deserialize_with = "deserialize_truthy")]
    zahir: bool,
}

/// Accept `true`/`false`, `1`/`0`, `yes`/`no` (case-insensitive) for query flags.
fn deserialize_truthy<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    Option::<String>::deserialize(deserializer)?
        .map(|s| match s.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" | "on" => Ok(true),
            "0" | "false" | "no" | "n" | "off" | "" => Ok(false),
            other => Err(D::Error::custom(format!(
                "invalid boolean {other:?}; expected 1/0 or true/false"
            ))),
        })
        .transpose()
        .map(|v| v.unwrap_or(false))
}

#[derive(Debug, Deserialize)]
pub(super) struct DeltaQuery {
    #[serde(rename = "type")]
    delta_type: Option<String>,
}

pub(super) async fn get_categories(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| Ok(Json(list_categories(conn)?)))
}

pub(super) async fn get_entries(
    State(state): State<AppState>,
    Query(q): Query<EntriesQuery>,
) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| Ok(Json(list_entries(conn, &q.filter())?)))
}

pub(super) async fn get_entry(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
    Query(q): Query<EntryQuery>,
) -> Result<Response, ApiError> {
    let path = require_rel_path(&path)?;
    let dir = current_dir(&state)?;
    with_db(&state, |conn| {
        let row = entry_row(conn, &path, q.zahir)?;
        if !q.zahir {
            return Ok(Json(row).into_response());
        }
        let typed = settings_api::effective_typed_column_tables(&dir);
        let (metadata_tables, writing_tables) = entry_table_views(row.zahir.as_ref(), typed);
        Ok(Json(EntryDetailResponse {
            row,
            metadata_tables,
            writing_tables,
        })
        .into_response())
    })
}

pub(super) fn entry_row(
    conn: &Connection,
    path: &str,
    include_zahir: bool,
) -> Result<EntryRow, ApiError> {
    match entry_detail(conn, path, include_zahir) {
        Ok(r) => Ok(r),
        Err(e) if is_not_found(&e) => Err(ApiError::not_found(e)),
        Err(e) => Err(ApiError::from(e)),
    }
}

#[derive(Debug, Serialize)]
struct EntryDetailResponse {
    #[serde(flatten)]
    row: EntryRow,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_tables: Option<Vec<SectionView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    writing_tables: Option<Vec<SectionView>>,
}

fn entry_table_views(
    zahir: Option<&serde_json::Value>,
    typed: crate::config::ColumnStatsDisplay,
) -> (Option<Vec<SectionView>>, Option<Vec<SectionView>>) {
    let Some(value) = zahir else {
        return (None, None);
    };
    let preview = sectioned_preview_from_zahir(value);
    let metadata_tables = preview.metadata.as_deref().and_then(|json| {
        let views = parse_json_to_views(json, typed);
        (!views.is_empty()).then_some(views)
    });
    let writing_tables = preview.writing.as_deref().and_then(|json| {
        let views = parse_json_to_views(json, typed);
        (!views.is_empty()).then_some(views)
    });
    (metadata_tables, writing_tables)
}

pub(super) async fn get_delta(
    State(state): State<AppState>,
    Query(q): Query<DeltaQuery>,
) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| {
        Ok(Json(list_delta(conn, q.delta_type.as_deref())?))
    })
}

pub(super) async fn get_duplicates(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let (dir, read_path) = with_inner(&state, |inner| {
        Ok((inner.catalog.dir.clone(), inner.catalog.read_path.clone()))
    })?;
    let body = list_duplicates(&read_path, &dir).map_err(ApiError::from)?;
    Ok(Json(body))
}

pub(super) async fn get_lenses(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| Ok(Json(list_lens_names(conn)?)))
}

pub(super) async fn get_lens(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Response, ApiError> {
    with_db(&state, |conn| {
        json_or_not_found(list_lens_entries(conn, &name))
    })
}

fn json_or_not_found<T: serde::Serialize>(
    result: Result<T, anyhow::Error>,
) -> Result<Response, ApiError> {
    match result {
        Ok(v) => Ok(Json(v).into_response()),
        Err(e) if is_not_found(&e) => Err(ApiError::not_found(e)),
        Err(e) => Err(ApiError::from(e)),
    }
}
