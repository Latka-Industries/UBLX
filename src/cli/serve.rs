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
    EntryListFilter, entry_detail, is_not_found, list_categories, list_delta, list_entries,
    list_lens_entries, list_lens_names,
};
use crate::cli::doctor;
use crate::cli_parser::ServeCli;
use crate::config::{UblxPaths, all_indexed_roots_alphabetical, record_prior_root_selected};
use crate::handlers::run_snap_pipeline_from_dir_db;

/// Live catalog for one serve process — switchable via `/roots/current`.
struct ServeCatalog {
    dir: PathBuf,
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
        .route("/delta", get(get_delta))
        .route("/lenses", get(get_lenses))
        .route("/lenses/{name}", get(get_lens))
        .with_state(state);

    tokio_rt::runtime().block_on(panza_run(
        ServeMeta {
            service: "ublx",
            version: env!("CARGO_PKG_VERSION"),
        },
        args.serve.clone(),
        api,
        StaticMount::None,
    ))
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
    let new_conn = handle.conn;
    with_inner(&state, |inner| {
        if same_dir(&inner.catalog.dir, &new_dir) {
            info!("serve root unchanged: {}", inner.catalog.dir.display());
        } else {
            let prev = inner.catalog.dir.display().to_string();
            inner.catalog.dir = new_dir.clone();
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
    let enhance_all = body.map(|j| j.0.enhance_all).unwrap_or(false);
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
        run_serve_snapshot_job(state_bg, dir, enhance_all);
    });

    let status = with_inner(&state, |inner| Ok(inner.snapshot.status()))?;
    Ok((StatusCode::ACCEPTED, Json(status)))
}

fn run_serve_snapshot_job(state: AppState, dir: PathBuf, enhance_all: bool) {
    let db_path = UblxPaths::new(&dir).db();
    let (tx, rx) = mpsc::channel();
    let preserve_enhance = enhance_all.then_some(Some(true));
    run_snap_pipeline_from_dir_db(&dir, &db_path, Some(tx), None, preserve_enhance, None);
    let (added, modified, removed) = rx.recv().unwrap_or((0, 0, 0));
    let last = SnapshotLast {
        added,
        modified,
        removed,
        error: None,
    };

    match open_catalog_for_read(&dir) {
        Ok(handle) => {
            let Ok(mut inner) = state.lock() else {
                warn!("serve snapshot finished but catalog lock poisoned");
                return;
            };
            // Only refresh conn if we are still on the same root (switch was blocked while running).
            if same_dir(&inner.catalog.dir, &dir) {
                inner.catalog.conn = handle.conn;
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
    with_db(&state, |conn| {
        json_or_not_found(entry_detail(conn, &path, q.zahir))
    })
}

async fn get_delta(
    State(state): State<AppState>,
    Query(q): Query<DeltaQuery>,
) -> Result<impl IntoResponse, ApiError> {
    with_db(&state, |conn| {
        Ok(Json(list_delta(conn, q.delta_type.as_deref())?))
    })
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
