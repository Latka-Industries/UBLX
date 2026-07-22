//! `ublx serve` — local HTTP API over `.ublx` (THI-156 / v0.1.13).

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
use crate::cli::settings_api::{self, SettingsPatch};
use crate::cli_parser::ServeCli;
use crate::config::{UblxPaths, all_indexed_roots_alphabetical, record_prior_root_selected};
use crate::handlers::run_snap_pipeline_from_dir_db;
use crate::handlers::viewing::sectioned_preview_from_zahir;
use crate::integrations::file_type_from_metadata_name;
use crate::render::kv_tables::{SectionView, parse_json_to_views};
use crate::utils::file_content_for_viewer;

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
    let path = normalize_entry_path(&path);
    let dir = current_dir(&state)?;
    with_db(&state, |conn| {
        let row = match entry_detail(conn, &path, q.zahir) {
            Ok(r) => r,
            Err(e) if is_not_found(&e) => return Err(ApiError::not_found(e)),
            Err(e) => return Err(ApiError::from(e)),
        };
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

#[derive(Debug, Deserialize)]
struct EntryContentQuery {
    /// `text` (default) or `html` (markdown → HTML via pulldown-cmark).
    #[serde(default)]
    format: Option<String>,
}

#[derive(Debug, Serialize)]
struct EntryContentResponse {
    path: String,
    category: String,
    /// `text` or `html`
    format: String,
    content: String,
}

async fn get_entry_content(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
    Query(q): Query<EntryContentQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let path = normalize_entry_path(&path);
    if path.is_empty() || path.contains("..") {
        return Err(ApiError::bad_request("invalid entry path"));
    }
    let dir = current_dir(&state)?;
    let row = with_db(&state, |conn| match entry_detail(conn, &path, false) {
        Ok(r) => Ok(r),
        Err(e) if is_not_found(&e) => Err(ApiError::not_found(e)),
        Err(e) => Err(ApiError::from(e)),
    })?;
    let abs = resolve_entry_disk_path(&dir, &path)?;
    let zahir_type = file_type_from_metadata_name(&row.category);
    let text = file_content_for_viewer(&abs, zahir_type).unwrap_or_else(|| "(empty)".into());

    // Explicit `html` always; default HTML only for Markdown; `text` always raw.
    let want_html = match q.format.as_deref().map(str::trim) {
        Some("html") => true,
        Some("text") => false,
        None => zahir_type == Some(crate::integrations::ZahirFT::Markdown),
        Some(other) => {
            return Err(ApiError::bad_request(format!(
                "invalid format {other:?}; expected text|html"
            )));
        }
    };

    let (format, content) = if want_html {
        ("html".into(), markdown_to_html(&text))
    } else {
        ("text".into(), text)
    };

    Ok(Json(EntryContentResponse {
        path: row.path,
        category: row.category,
        format,
        content,
    }))
}

fn markdown_to_html(src: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let parser = Parser::new_ext(src, Options::all());
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
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
    let view = settings_api::patch_settings(&dir, &scope, patch).map_err(ApiError::bad_request)?;
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
