//! Windowed plain-text `/content` responses (`offset`/`limit`).

use std::path::Path;

use axum::Json;
use axum::response::{IntoResponse, Response};

use crate::cli::catalog_read::EntryRow;
use crate::utils::read_text_byte_window;

use super::super::error::ApiError;
use super::EntryContentResponse;

/// `GET /content/{path}?format=text&offset=N&limit=M` — explore windowing for large text bodies.
pub(super) fn windowed_text_content_response(
    row: &EntryRow,
    abs: &Path,
    offset: u64,
    limit: u64,
    format: Option<&str>,
) -> Result<Response, ApiError> {
    match format.map(str::trim) {
        None | Some("text") => {}
        Some("html") => {
            return Err(ApiError::bad_request(
                "offset/limit windows are format=text only (HTML/syntect needs full-body or a later slice pipeline)",
            ));
        }
        Some(other) => {
            return Err(ApiError::bad_request(format!(
                "offset/limit not supported with format {other:?}"
            )));
        }
    }
    if limit == 0 {
        return Err(ApiError::bad_request("limit must be > 0"));
    }
    let win = read_text_byte_window(abs, offset, limit).ok_or_else(|| {
        ApiError::bad_request("could not read byte window (missing file or not a regular file)")
    })?;
    Ok(Json(EntryContentResponse {
        path: row.path.clone(),
        category: row.category.clone(),
        format: "text".into(),
        content: win.text,
        page: None,
        page_count: None,
        offset: Some(win.offset),
        byte_len: Some(win.byte_len),
        limit: Some(limit),
        total_bytes: Some(win.total),
        tree: None,
    })
    .into_response())
}
