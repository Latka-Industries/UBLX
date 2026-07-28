//! Catalog list/detail routes: entries, categories, delta, lenses, duplicates.

use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use axum::response::{IntoResponse, Response};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::cli::catalog_read::{
    EntryListFilter, EntryListWindow, EntryRow, entry_detail, is_not_found, list_categories,
    list_delta, list_duplicates, list_entries, list_entries_page, list_lens_entries,
    list_lens_names,
};
use crate::cli::settings_api;
use crate::handlers::sectioned_preview_from_zahir;
use crate::render::kv_tables::{SectionView, parse_json_to_views};
use crate::render::templates::{TemplateView, template_views_from_value};

use super::content::paths::require_rel_path;
use super::error::ApiError;
use super::state::{AppState, current_dir, with_db, with_inner};

#[derive(Debug, Deserialize)]
pub(super) struct EntriesQuery {
    category: Option<String>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    contains: Option<String>,
    /// When set, response is `{ total, offset, limit, entries }` instead of a bare array (THI-205).
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
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
) -> Result<Response, ApiError> {
    with_db(&state, |conn| {
        let filter = q.filter();
        if let Some(limit) = q.limit {
            let page = list_entries_page(
                conn,
                &filter,
                EntryListWindow {
                    offset: q.offset.unwrap_or(0),
                    limit,
                },
            )?;
            Ok(Json(page).into_response())
        } else {
            // Legacy: bare array for existing web / remote clients (THI-207 will pass limit).
            Ok(Json(list_entries(conn, &filter)?).into_response())
        }
    })
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
        let views = entry_structured_views(row.zahir.as_ref(), typed);
        Ok(Json(EntryDetailResponse {
            row,
            metadata_tables: views.metadata_tables,
            writing_tables: views.writing_tables,
            template_views: views.template_views,
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
    /// Host-parsed Templates (`TemplateView`); present when `?zahir=1` and non-empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    template_views: Option<Vec<TemplateView>>,
}

fn entry_structured_views(
    zahir: Option<&serde_json::Value>,
    typed: crate::config::ColumnStatsDisplay,
) -> StructuredEntryViews {
    let Some(value) = zahir else {
        return StructuredEntryViews::default();
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
    let template_views = {
        let views = template_views_from_value(value);
        (!views.is_empty()).then_some(views)
    };
    StructuredEntryViews {
        metadata_tables,
        writing_tables,
        template_views,
    }
}

#[derive(Default)]
struct StructuredEntryViews {
    metadata_tables: Option<Vec<SectionView>>,
    writing_tables: Option<Vec<SectionView>>,
    template_views: Option<Vec<TemplateView>>,
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
