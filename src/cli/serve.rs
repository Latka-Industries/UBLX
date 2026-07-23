//! `ublx serve` — local HTTP API over `.ublx` (THI-156 / v0.1.13).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use log::{info, warn};
use panza::{ServeMeta, StaticMount, run as panza_run};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::app::tokio_rt;
use crate::cli::catalog::open_catalog_for_read;
use crate::cli::catalog_read::{
    EntryListFilter, EntryRow, entry_detail, is_not_found, list_categories, list_delta,
    list_duplicates, list_entries, list_lens_entries, list_lens_names,
};
use crate::cli::doctor;
use crate::cli::remote::encode_entry_path;
use crate::cli::settings_api::{self, SettingsPatch};
use crate::cli_parser::ServeCli;
use crate::config::{UblxPaths, all_indexed_roots_alphabetical, record_prior_root_selected};
use crate::handlers::run_snap_pipeline_from_dir_db;
use crate::handlers::viewing::{directory_tree_nodes, sectioned_preview_from_zahir};
use crate::integrations::{ZahirFT, delimiter_from_path_for_viewer, file_type_from_metadata_name};
use crate::render::kv_tables::{
    SectionView, TreeNodeView, parse_json_to_views, tree_node_to_view, tree_roots_to_lines,
};
use crate::render::viewers::{
    csv_handler, html_escape_minimal, images, pdf_preview, svg_preview, syntect_text, video_preview,
};
use crate::utils::{file_content_for_viewer, read_text_byte_window, try_extract_cover};

/// Live catalog for one serve process — switchable via `/roots/current`.
struct ServeCatalog {
    dir: PathBuf,
    /// Catalog file actually opened (for duplicate load and reopen).
    read_path: PathBuf,
    conn: Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum SnapshotState {
    Idle,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
struct SnapshotLast {
    added: usize,
    modified: usize,
    removed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SnapshotStatus {
    state: SnapshotState,
    #[serde(skip_serializing_if = "Option::is_none")]
    dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last: Option<SnapshotLast>,
}

struct SnapshotJob {
    state: SnapshotState,
    dir: Option<PathBuf>,
    last: Option<SnapshotLast>,
}

impl SnapshotJob {
    fn idle() -> Self {
        Self {
            state: SnapshotState::Idle,
            dir: None,
            last: None,
        }
    }

    fn status(&self) -> SnapshotStatus {
        SnapshotStatus {
            state: self.state,
            dir: self.dir.as_ref().map(|p| p.display().to_string()),
            last: self.last.clone(),
        }
    }

    fn is_running(&self) -> bool {
        self.state == SnapshotState::Running
    }

    fn mark_finished(&mut self, state: SnapshotState, last: SnapshotLast) {
        self.state = state;
        self.last = Some(last);
    }
}

struct AppStateInner {
    catalog: ServeCatalog,
    snapshot: SnapshotJob,
}

type AppState = Arc<Mutex<AppStateInner>>;

/// Run `ublx serve` until the process is interrupted.
///
/// # Errors
///
/// Returns `Err` when the catalog cannot be opened or the server fails to bind.
pub fn run(args: &ServeCli) -> Result<(), anyhow::Error> {
    let handle = open_catalog_for_read(&args.dir)?;
    info!(
        "serve catalog ready: dir={} db={}",
        handle.paths.dir.display(),
        handle.read_path.display()
    );
    let state: AppState = Arc::new(Mutex::new(AppStateInner {
        catalog: ServeCatalog {
            dir: handle.paths.dir,
            read_path: handle.read_path,
            conn: handle.conn,
        },
        snapshot: SnapshotJob::idle(),
    }));

    let api = Router::new()
        .route("/roots", get(get_roots))
        .route(
            "/roots/current",
            get(get_current_root).put(put_current_root),
        )
        .route("/doctor", get(get_doctor))
        .route("/snapshot", get(get_snapshot).post(post_snapshot))
        .route("/categories", get(get_categories))
        .route("/entries", get(get_entries))
        .route("/entries/{*path}", get(get_entry))
        .route("/content/{*path}", get(get_entry_content))
        .route("/delta", get(get_delta))
        .route("/duplicates", get(get_duplicates))
        .route("/lenses", get(get_lenses))
        .route("/lenses/{name}", get(get_lens))
        .route(
            "/settings/{scope}",
            get(get_settings).patch(patch_settings_route),
        )
        .with_state(state);

    tokio_rt::runtime().block_on(panza_run(
        ServeMeta {
            service: "ublx",
            version: env!("CARGO_PKG_VERSION"),
        },
        args.serve.clone(),
        api,
        static_mount(),
    ))
}

/// Optional Leptos UI (`--features ui`): serve `crates/ublx-web/dist` (or `UBLX_WEB_DIST`).
fn static_mount() -> StaticMount {
    #[cfg(feature = "ui")]
    {
        let dir = std::env::var_os("UBLX_WEB_DIST")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/ublx-web/dist"));
        if !dir.join("index.html").is_file() {
            warn!(
                "feature `ui` enabled but {}/index.html missing — run crates/ublx-web/build.sh",
                dir.display()
            );
        } else {
            info!("serve UI static mount: {}", dir.display());
        }
        StaticMount::Dir(dir)
    }
    #[cfg(not(feature = "ui"))]
    {
        StaticMount::None
    }
}

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
struct SwitchRootBody {
    /// Absolute or relative path to an indexed project root (same as TUI switch).
    dir: String,
}

#[derive(Debug, Default, Deserialize)]
struct SnapshotBody {
    /// When true, force full Zahir enhance for this run (same idea as CLI enhance-all).
    #[serde(default)]
    enhance_all: bool,
}

#[derive(Debug, Deserialize)]
struct EntriesQuery {
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
struct EntryQuery {
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
struct DeltaQuery {
    #[serde(rename = "type")]
    delta_type: Option<String>,
}

async fn get_roots(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
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

async fn get_current_root(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(CurrentRoot {
        path: current_dir(&state)?.display().to_string(),
    }))
}

async fn put_current_root(
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

async fn get_doctor(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let dir = current_dir(&state)?;
    let report = doctor::diagnose(&dir).map_err(ApiError::from)?;
    info!(
        "serve doctor: dir={} summary={:?}",
        report.dir, report.summary
    );
    Ok(Json(report))
}

async fn get_snapshot(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    with_inner(&state, |inner| Ok(Json(inner.snapshot.status())))
}

async fn post_snapshot(
    State(state): State<AppState>,
    body: Option<Json<SnapshotBody>>,
) -> Result<impl IntoResponse, ApiError> {
    let enhance_all = body.is_some_and(|j| j.0.enhance_all);
    let dir = with_inner(&state, |inner| {
        if inner.snapshot.is_running() {
            return Err(ApiError::conflict("snapshot already running"));
        }
        let dir = inner.catalog.dir.clone();
        inner.snapshot.state = SnapshotState::Running;
        inner.snapshot.dir = Some(dir.clone());
        Ok(dir)
    })?;

    info!(
        "serve snapshot started: dir={} enhance_all={enhance_all}",
        dir.display()
    );
    let state_bg = Arc::clone(&state);
    std::thread::spawn(move || {
        run_serve_snapshot_job(&state_bg, &dir, enhance_all);
    });

    let status = with_inner(&state, |inner| Ok(inner.snapshot.status()))?;
    Ok((StatusCode::ACCEPTED, Json(status)))
}

fn run_serve_snapshot_job(state: &AppState, dir: &Path, enhance_all: bool) {
    let db_path = UblxPaths::new(dir).db();
    let (tx, rx) = mpsc::channel();
    let preserve_enhance = enhance_all.then_some(Some(true));
    run_snap_pipeline_from_dir_db(dir, &db_path, Some(tx), None, preserve_enhance, None);
    let (added, modified, removed) = rx.recv().unwrap_or((0, 0, 0));
    let last = SnapshotLast {
        added,
        modified,
        removed,
        error: None,
    };

    match open_catalog_for_read(dir) {
        Ok(handle) => {
            let Ok(mut inner) = state.lock() else {
                warn!("serve snapshot finished but catalog lock poisoned");
                return;
            };
            // Only refresh conn if we are still on the same root (switch was blocked while running).
            if same_dir(&inner.catalog.dir, dir) {
                inner.catalog.conn = handle.conn;
                inner.catalog.read_path = handle.read_path;
            }
            inner.snapshot.mark_finished(SnapshotState::Done, last);
            info!(
                "serve snapshot finished: dir={} +{added} ~{modified} -{removed}",
                dir.display()
            );
        }
        Err(e) => {
            warn!(
                "serve snapshot finished but reopen failed for {}: {e}",
                dir.display()
            );
            if let Ok(mut inner) = state.lock() {
                inner.snapshot.mark_finished(
                    SnapshotState::Failed,
                    SnapshotLast {
                        added,
                        modified,
                        removed,
                        error: Some(e.to_string()),
                    },
                );
            }
        }
    }
}

async fn get_categories(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| Ok(Json(list_categories(conn)?)))
}

async fn get_entries(
    State(state): State<AppState>,
    Query(q): Query<EntriesQuery>,
) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| Ok(Json(list_entries(conn, &q.filter())?)))
}

async fn get_entry(
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

/// Cap for passing through original bytes (`format=raw`). Larger / non-web formats → PNG preview.
const MAX_RAW_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct EntryContentQuery {
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
struct EntryContentResponse {
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

async fn get_entry_content(
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
        return directory_tree_content_response(&row, &abs);
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

fn directory_tree_content_response(row: &EntryRow, abs: &Path) -> Result<Response, ApiError> {
    let roots = directory_tree_nodes(abs);
    let tree: Vec<TreeNodeView> = roots.iter().map(tree_node_to_view).collect();
    let content = tree_roots_to_lines(&roots).join("\n");
    Ok(Json(EntryContentResponse {
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
    .into_response())
}

/// `GET /content/{path}?format=text&offset=N&limit=M` — explore windowing for large text bodies.
fn windowed_text_content_response(
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

fn content_to_html(
    text: &str,
    path: &str,
    abs: &Path,
    zahir_type: Option<ZahirFT>,
    appearance: crate::themes::Appearance,
    page: Option<u32>,
) -> String {
    match zahir_type {
        Some(ZahirFT::Markdown) => markdown_to_html(text),
        Some(ft) if syntect_text::uses_syntect_ft(ft) => {
            syntect_text::highlight_viewer_html(text, path, ft, appearance)
        }
        Some(ZahirFT::Csv) => csv_handler::delimited_to_html(text, path),
        Some(ZahirFT::Text) => format!("<pre>{}</pre>", html_escape_minimal(text)),
        Some(ZahirFT::Image) => image_viewer_html(path, abs, None),
        Some(ZahirFT::Pdf) => image_viewer_html(path, abs, Some(("raw", page.unwrap_or(1)))),
        Some(ZahirFT::Video) => image_viewer_html(path, abs, Some(("raw", 0))),
        Some(ft @ (ZahirFT::Audio | ZahirFT::Epub)) => {
            if try_extract_cover(abs, ft).is_some() {
                image_preview_html(path, "cover", None)
            } else {
                r#"<p class="img-viewer__empty">(no embedded cover)</p>"#.into()
            }
        }
        _ if svg_preview::is_svg_path(Path::new(path)) => image_viewer_html(path, abs, None),
        _ if delimiter_from_path_for_viewer(path).is_some() => {
            csv_handler::delimited_to_html(text, path)
        }
        _ => format!("<pre>{}</pre>", html_escape_minimal(text)),
    }
}

/// `<img>` plus an in-pane note when preview already fails (TIFF / missing tools / etc.).
///
/// `raw_opts`: `Some(("raw", page))` for PDF (`page` 1-based); `Some(("raw", 0))` for video;
/// `None` for Image/SVG (plain `?format=raw`).
fn image_viewer_html(rel_path: &str, abs: &Path, raw_opts: Option<(&str, u32)>) -> String {
    let img = match raw_opts {
        Some(("raw", page)) if page >= 1 => image_preview_html(rel_path, "raw", Some(page)),
        Some(("raw", _)) | None => image_preview_html(rel_path, "raw", None),
        Some((fmt, _)) => image_preview_html(rel_path, fmt, None),
    };
    let check = match raw_opts {
        Some(("raw", page)) if page >= 1 => ensure_tool_previewable(abs, Some(ZahirFT::Pdf), page),
        Some(("raw", _)) => ensure_tool_previewable(abs, Some(ZahirFT::Video), 1),
        _ => ensure_image_previewable(abs),
    };
    match check {
        Ok(()) => img,
        Err(msg) => format!(
            r#"{img}<p class="img-viewer__empty">{}</p>"#,
            html_escape_minimal(&msg)
        ),
    }
}

fn image_preview_html(rel_path: &str, format_query: &str, page: Option<u32>) -> String {
    let mut src = format!(
        "/content/{}?format={format_query}",
        encode_entry_path(rel_path)
    );
    if let Some(p) = page {
        let _ = write!(src, "&page={p}");
    }
    let alt = html_escape_minimal(rel_path);
    format!(
        r#"<div class="img-viewer"><img class="img-viewer__img" src="{src}" alt="{alt}" loading="lazy" /></div>"#
    )
}

fn ensure_image_previewable(abs: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(abs).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    if svg_preview::is_svg_path(abs) {
        if meta.len() > MAX_RAW_IMAGE_BYTES {
            return Err(format!("image larger than {MAX_RAW_IMAGE_BYTES} bytes"));
        }
        return Ok(());
    }
    if needs_png_preview(abs, meta.len()) {
        decode_png_preview(abs, meta.len()).map(|_| ())
    } else {
        Ok(())
    }
}

fn ensure_tool_previewable(
    abs: &Path,
    zahir_type: Option<ZahirFT>,
    page: u32,
) -> Result<(), String> {
    let meta = std::fs::metadata(abs).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    match zahir_type {
        Some(ZahirFT::Pdf) => decode_pdf_preview(abs, page, meta.len()).map(|_| ()),
        Some(ZahirFT::Video) => decode_video_preview(abs, meta.len()).map(|_| ()),
        _ => Err("not a tool-backed preview category".into()),
    }
}

fn allows_raw_media(zahir_type: Option<ZahirFT>, abs: &Path) -> bool {
    matches!(
        zahir_type,
        Some(ZahirFT::Image | ZahirFT::Pdf | ZahirFT::Video)
    ) || svg_preview::is_svg_path(abs)
}

fn raw_media_response(
    abs: &Path,
    zahir_type: Option<ZahirFT>,
    page: Option<u32>,
) -> Result<Response, ApiError> {
    if !allows_raw_media(zahir_type, abs) {
        return Err(ApiError::bad_request(
            "format=raw is only for Image, PDF, Video (or .svg) entries",
        ));
    }
    let meta = std::fs::metadata(abs).map_err(ApiError::not_found)?;
    if !meta.is_file() {
        return Err(ApiError::bad_request("not a file"));
    }
    match zahir_type {
        Some(ZahirFT::Pdf) => {
            return png_bytes_response(decode_pdf_preview(
                abs,
                page.unwrap_or(1).max(1),
                meta.len(),
            ));
        }
        Some(ZahirFT::Video) => return png_bytes_response(decode_video_preview(abs, meta.len())),
        _ => {}
    }
    // SVG stays vector; browsers can't show TIFF and many BMPs/huge rasters need a PNG preview.
    if svg_preview::is_svg_path(abs) {
        if meta.len() > MAX_RAW_IMAGE_BYTES {
            return Err(ApiError::bad_request(format!(
                "image larger than {MAX_RAW_IMAGE_BYTES} bytes"
            )));
        }
        let bytes = std::fs::read(abs).map_err(ApiError::not_found)?;
        return Ok(([(axum::http::header::CONTENT_TYPE, "image/svg+xml")], bytes).into_response());
    }
    if needs_png_preview(abs, meta.len()) {
        return png_preview_response(abs, meta.len());
    }
    let bytes = std::fs::read(abs).map_err(ApiError::not_found)?;
    let mime = image_mime_from_path(abs);
    Ok(([(axum::http::header::CONTENT_TYPE, mime)], bytes).into_response())
}

/// TIFF (no browser `<img>`), BMP/ICO, and oversize files → decode + PNG (TUI-tiered downscale).
fn needs_png_preview(path: &Path, len: u64) -> bool {
    if len > MAX_RAW_IMAGE_BYTES {
        return true;
    }
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("tif" | "tiff" | "bmp" | "dib" | "ico" | "tga")
    )
}

fn png_preview_response(abs: &Path, file_size: u64) -> Result<Response, ApiError> {
    png_bytes_response(decode_png_preview(abs, file_size))
}

fn png_bytes_response(result: Result<Vec<u8>, String>) -> Result<Response, ApiError> {
    let bytes = result.map_err(ApiError::bad_request)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], bytes).into_response())
}

fn encode_png_bytes(img: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("encode png: {e}"))?;
    Ok(buf.into_inner())
}

fn decode_png_preview(abs: &Path, file_size: u64) -> Result<Vec<u8>, String> {
    let max_dim = images::tiered_max_dimension_for_file_size(file_size);
    let img = image::open(abs).map_err(|e| format!("decode image: {e}"))?;
    encode_png_bytes(&images::downscale_with_max(img, max_dim))
}

fn decode_pdf_preview(abs: &Path, page: u32, file_size: u64) -> Result<Vec<u8>, String> {
    let max_dim = pdf_preview::PdfRasterMaxDimBoost::apply(
        images::tiered_max_dimension_for_file_size(file_size),
    );
    let img = pdf_preview::render_pdf_page(abs, page, max_dim)?;
    encode_png_bytes(&images::downscale_with_max(img, max_dim))
}

fn decode_video_preview(abs: &Path, file_size: u64) -> Result<Vec<u8>, String> {
    let max_dim = images::tiered_max_dimension_for_file_size(file_size);
    let img = video_preview::decode_preview_frame(abs)?;
    encode_png_bytes(&images::downscale_with_max(img, max_dim))
}

fn embedded_cover_response(abs: &Path, zahir_type: Option<ZahirFT>) -> Result<Response, ApiError> {
    let Some(ft @ (ZahirFT::Audio | ZahirFT::Epub)) = zahir_type else {
        return Err(ApiError::bad_request(
            "format=cover is only for Audio or Epub entries",
        ));
    };
    let Some(bytes) = try_extract_cover(abs, ft) else {
        return Err(ApiError::not_found("no embedded cover"));
    };
    // Covers may be JPEG/PNG (pass through) or uncommon codecs → PNG preview.
    let mime = image_mime_from_bytes(&bytes);
    if matches!(
        mime,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    ) {
        return Ok(([(axum::http::header::CONTENT_TYPE, mime)], bytes).into_response());
    }
    let img = image::load_from_memory(&bytes)
        .map_err(|e| ApiError::bad_request(format!("decode cover: {e}")))?;
    let img = images::downscale_with_max(
        img,
        images::tiered_max_dimension_for_file_size(bytes.len() as u64),
    );
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| ApiError::bad_request(format!("encode cover png: {e}")))?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        buf.into_inner(),
    )
        .into_response())
}

fn image_mime_from_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        Some("svg") => "image/svg+xml",
        Some("avif") => "image/avif",
        Some("tif" | "tiff") => "image/tiff",
        _ => "application/octet-stream",
    }
}

fn image_mime_from_bytes(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        let head = std::str::from_utf8(&bytes[..bytes.len().min(256)]).unwrap_or("");
        let trimmed = head.trim_start();
        if trimmed.starts_with("<svg")
            || trimmed.starts_with("<SVG")
            || trimmed.starts_with("<?xml")
        {
            "image/svg+xml"
        } else {
            "application/octet-stream"
        }
    }
}

fn markdown_to_html(src: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let parser = Parser::new_ext(src, Options::all());
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn entry_row(conn: &Connection, path: &str, include_zahir: bool) -> Result<EntryRow, ApiError> {
    match entry_detail(conn, path, include_zahir) {
        Ok(r) => Ok(r),
        Err(e) if is_not_found(&e) => Err(ApiError::not_found(e)),
        Err(e) => Err(ApiError::from(e)),
    }
}

/// Normalize and reject empty / `..` segments (path-traversal).
fn require_rel_path(path: &str) -> Result<String, ApiError> {
    let path = normalize_entry_path(path);
    if path.is_empty() || path.split('/').any(|s| s == "..") {
        return Err(ApiError::bad_request("invalid entry path"));
    }
    Ok(path)
}

/// Join catalog-relative path under the current root; reject escapes outside the root.
fn resolve_entry_disk_path(root: &Path, rel: &str) -> Result<PathBuf, ApiError> {
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

async fn get_delta(
    State(state): State<AppState>,
    Query(q): Query<DeltaQuery>,
) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| {
        Ok(Json(list_delta(conn, q.delta_type.as_deref())?))
    })
}

async fn get_duplicates(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (dir, read_path) = with_inner(&state, |inner| {
        Ok((inner.catalog.dir.clone(), inner.catalog.read_path.clone()))
    })?;
    let body = list_duplicates(&read_path, &dir).map_err(ApiError::from)?;
    Ok(Json(body))
}

async fn get_lenses(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| Ok(Json(list_lens_names(conn)?)))
}

async fn get_lens(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Response, ApiError> {
    with_db(&state, |conn| {
        json_or_not_found(list_lens_entries(conn, &name))
    })
}

async fn get_settings(
    State(state): State<AppState>,
    AxumPath(scope): AxumPath<String>,
) -> Result<impl IntoResponse, ApiError> {
    let dir = current_dir(&state)?;
    let view = settings_api::get_settings_view(&dir, &scope).map_err(ApiError::bad_request)?;
    Ok(Json(view))
}

async fn patch_settings_route(
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

fn with_db<T>(
    state: &AppState,
    f: impl FnOnce(&Connection) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    with_inner(state, |inner| f(&inner.catalog.conn))
}

fn with_inner<T>(
    state: &AppState,
    f: impl FnOnce(&mut AppStateInner) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let mut inner = state.lock().map_err(|_| ApiError::lock())?;
    f(&mut inner)
}

fn current_dir(state: &AppState) -> Result<PathBuf, ApiError> {
    with_inner(state, |inner| Ok(inner.catalog.dir.clone()))
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

/// Axum may leave a leading slash on `{*path}`; catalog paths are relative without it.
fn normalize_entry_path(path: &str) -> String {
    path.trim_start_matches('/').to_string()
}

fn canonicalize_dir(dir: &Path) -> PathBuf {
    dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf())
}

fn same_dir(a: &Path, b: &Path) -> bool {
    canonicalize_dir(a) == canonicalize_dir(b)
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn lock() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "catalog lock poisoned".into(),
        }
    }

    fn not_found(err: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: err.to_string(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        let msg = err.to_string();
        let status = if msg.contains("no catalog DB") || msg.contains("not a directory") {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::BAD_REQUEST
        };
        Self {
            status,
            message: msg,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Access log shows status only; spell out the reason for 4xx/5xx bodies.
        if self.status.is_server_error() {
            warn!("serve {}: {}", self.status, self.message);
        } else if self.status != StatusCode::NOT_FOUND {
            info!("serve {}: {}", self.status, self.message);
        }
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}
