//! `/content/{*path}` — text/html/raw/cover/windowed/tree.

mod html;
mod media;
pub(super) mod paths;
mod window;

use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::cli::catalog_read::EntryRow;
use crate::cli::settings_api;
use crate::handlers::viewing::directory_tree_nodes;
use crate::integrations::{ZahirFT, delimiter_from_path_for_viewer, file_type_from_metadata_name};
use crate::render::kv_tables::{TreeNodeView, tree_node_to_view, tree_roots_to_lines};
use crate::render::viewers::{pdf_preview, svg_preview, syntect_text};
use crate::utils::file_content_for_viewer;

use super::catalog::entry_row;
use super::error::ApiError;
use super::state::{AppState, current_dir, with_db};

use self::html::content_to_html;
use self::media::{embedded_cover_response, raw_media_response};
use self::paths::{require_rel_path, resolve_entry_disk_path};
use self::window::windowed_text_content_response;

#[derive(Debug, Deserialize)]
pub(super) struct EntryContentQuery {
    /// `text` | `html` | `raw` (image / PDF page / video frame) | `cover` (Audio/Epub art).
    /// Omitted → HTML for Markdown / syntect / CSV / Text / Image / PDF / Video / cover cats.
    #[serde(default)]
    format: Option<String>,
    /// PDF page (1-based) for `format=raw` / HTML preview. Default `1`.
    #[serde(default)]
    page: Option<u32>,
    /// Byte offset into the file for a plain-text window (`format=text` only). Explore #12.
    #[serde(default)]
    offset: Option<u64>,
    /// Max bytes to return from `offset` (`format=text` only). Caps at half MiB. Explore #12.
    #[serde(default)]
    limit: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(super) struct EntryContentResponse {
    path: String,
    category: String,
    /// `text` or `html`
    format: String,
    content: String,
    /// PDF page shown in `content` / requested for `raw` (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u32>,
    /// PDF page count from `pdfinfo` when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    page_count: Option<u32>,
    /// Windowed text: start byte in file.
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<u64>,
    /// Windowed text: bytes read (may be < `limit` near EOF).
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_len: Option<u64>,
    /// Windowed text: requested limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u64>,
    /// Windowed text: file size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    total_bytes: Option<u64>,
    /// Directory / folder Viewer: nested tree for web collapse.
    #[serde(skip_serializing_if = "Option::is_none")]
    tree: Option<Vec<TreeNodeView>>,
}

pub(super) async fn get_entry_content(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
    Query(q): Query<EntryContentQuery>,
) -> Result<Response, ApiError> {
    let path = require_rel_path(&path)?;
    let dir = current_dir(&state)?;
    let row = with_db(&state, |conn| entry_row(conn, &path, false))?;
    let abs = resolve_entry_disk_path(&dir, &path)?;
    let zahir_type = file_type_from_metadata_name(&row.category);

    match q.format.as_deref().map(str::trim) {
        Some("raw") => return raw_media_response(&abs, zahir_type, q.page),
        Some("cover") => return embedded_cover_response(&abs, zahir_type),
        _ => {}
    }

    // Explore #12: explicit byte windows for plain text (not HTML / syntect).
    if let (Some(offset), Some(limit)) = (q.offset, q.limit) {
        return windowed_text_content_response(&row, &abs, offset, limit, q.format.as_deref());
    }

    // Directory / Zarr store path: structured tree for web (TUI still uses `tree` text).
    if abs.is_dir() {
        return Ok(directory_tree_content_response(&row, &abs));
    }

    let text = file_content_for_viewer(&abs, zahir_type).unwrap_or_else(|| "(empty)".into());
    let want_html = content_want_html(q.format.as_deref(), zahir_type, &row.path)?;
    let pdf_page = q.page.unwrap_or(1).max(1);
    let (format, content) = if want_html {
        let appearance = settings_api::effective_appearance(&dir);
        (
            "html".into(),
            content_to_html(
                &text,
                &row.path,
                &abs,
                zahir_type,
                appearance,
                Some(pdf_page),
            ),
        )
    } else {
        ("text".into(), text)
    };

    let (page, page_count) = if zahir_type == Some(ZahirFT::Pdf) {
        (Some(pdf_page), pdf_preview::pdf_page_count(&abs).ok())
    } else {
        (None, None)
    };

    Ok(Json(EntryContentResponse {
        path: row.path,
        category: row.category,
        format,
        content,
        page,
        page_count,
        offset: None,
        byte_len: None,
        limit: None,
        total_bytes: None,
        tree: None,
    })
    .into_response())
}

fn directory_tree_content_response(row: &EntryRow, abs: &Path) -> Response {
    let roots = directory_tree_nodes(abs);
    let tree: Vec<TreeNodeView> = roots.iter().map(tree_node_to_view).collect();
    let content = tree_roots_to_lines(&roots).join("\n");
    Json(EntryContentResponse {
        path: row.path.clone(),
        category: row.category.clone(),
        format: "tree".into(),
        content,
        page: None,
        page_count: None,
        offset: None,
        byte_len: None,
        limit: None,
        total_bytes: None,
        tree: Some(tree),
    })
    .into_response()
}

fn content_want_html(
    format: Option<&str>,
    zahir_type: Option<ZahirFT>,
    path: &str,
) -> Result<bool, ApiError> {
    match format.map(str::trim) {
        Some("html") => Ok(true),
        Some("text") => Ok(false),
        None => Ok(matches!(
            zahir_type,
            Some(
                ZahirFT::Markdown
                    | ZahirFT::Csv
                    | ZahirFT::Text
                    | ZahirFT::Image
                    | ZahirFT::Pdf
                    | ZahirFT::Video
                    | ZahirFT::Audio
                    | ZahirFT::Epub
            )
        ) || zahir_type.is_some_and(syntect_text::uses_syntect_ft)
            || delimiter_from_path_for_viewer(path).is_some()
            || svg_preview::is_svg_path(Path::new(path))),
        Some(other) => Err(ApiError::bad_request(format!(
            "invalid format {other:?}; expected text|html|raw|cover"
        ))),
    }
}
