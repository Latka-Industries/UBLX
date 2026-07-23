//! `/snapshot` status + trigger.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use log::{info, warn};
use serde::Deserialize;

use crate::cli::catalog::open_catalog_for_read;
use crate::config::UblxPaths;
use crate::handlers::run_snap_pipeline_from_dir_db;

use super::error::ApiError;
use super::state::{AppState, SnapshotLast, SnapshotState, same_dir, with_inner};

#[derive(Debug, Default, Deserialize)]
pub(super) struct SnapshotBody {
    /// When true, force full Zahir enhance for this run (same idea as CLI enhance-all).
    #[serde(default)]
    enhance_all: bool,
}

pub(super) async fn get_snapshot(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    with_inner(&state, |inner| Ok(Json(inner.snapshot.status())))
}

pub(super) async fn post_snapshot(
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
