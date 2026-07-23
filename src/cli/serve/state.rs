//! Live catalog + snapshot job state for one serve process.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::Serialize;

use super::error::ApiError;

/// Live catalog for one serve process — switchable via `/roots/current`.
pub(super) struct ServeCatalog {
    pub(super) dir: PathBuf,
    /// Catalog file actually opened (for duplicate load and reopen).
    pub(super) read_path: PathBuf,
    pub(super) conn: Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum SnapshotState {
    Idle,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SnapshotLast {
    pub(super) added: usize,
    pub(super) modified: usize,
    pub(super) removed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SnapshotStatus {
    state: SnapshotState,
    #[serde(skip_serializing_if = "Option::is_none")]
    dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last: Option<SnapshotLast>,
}

pub(super) struct SnapshotJob {
    pub(super) state: SnapshotState,
    pub(super) dir: Option<PathBuf>,
    last: Option<SnapshotLast>,
}

impl SnapshotJob {
    pub(super) fn idle() -> Self {
        Self {
            state: SnapshotState::Idle,
            dir: None,
            last: None,
        }
    }

    pub(super) fn status(&self) -> SnapshotStatus {
        SnapshotStatus {
            state: self.state,
            dir: self.dir.as_ref().map(|p| p.display().to_string()),
            last: self.last.clone(),
        }
    }

    pub(super) fn is_running(&self) -> bool {
        self.state == SnapshotState::Running
    }

    pub(super) fn mark_finished(&mut self, state: SnapshotState, last: SnapshotLast) {
        self.state = state;
        self.last = Some(last);
    }
}

pub(super) struct AppStateInner {
    pub(super) catalog: ServeCatalog,
    pub(super) snapshot: SnapshotJob,
}

pub(super) type AppState = Arc<Mutex<AppStateInner>>;

pub(super) fn with_db<T>(
    state: &AppState,
    f: impl FnOnce(&Connection) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    with_inner(state, |inner| f(&inner.catalog.conn))
}

pub(super) fn with_inner<T>(
    state: &AppState,
    f: impl FnOnce(&mut AppStateInner) -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let mut inner = state.lock().map_err(|_| ApiError::lock())?;
    f(&mut inner)
}

pub(super) fn current_dir(state: &AppState) -> Result<PathBuf, ApiError> {
    with_inner(state, |inner| Ok(inner.catalog.dir.clone()))
}

pub(super) fn canonicalize_dir(dir: &Path) -> PathBuf {
    dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf())
}

pub(super) fn same_dir(a: &Path, b: &Path) -> bool {
    canonicalize_dir(a) == canonicalize_dir(b)
}
