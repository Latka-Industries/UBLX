//! Roots, snapshot, and export routes.

use serde::{Deserialize, Serialize};

use super::catalog::CurrentRoot;
use super::http::{get_json, post_json, put_json};

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct RootRow {
    pub path: String,
    pub current: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct SnapshotLast {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct SnapshotStatus {
    /// `idle` | `running` | `done` | `failed`
    pub state: String,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub last: Option<SnapshotLast>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct ExportOut {
    pub count: usize,
    pub dir: String,
}

pub(crate) async fn fetch_roots() -> Result<Vec<RootRow>, String> {
    get_json("/roots").await
}

pub(crate) async fn switch_root(dir: &str) -> Result<CurrentRoot, String> {
    #[derive(Serialize)]
    struct Body<'a> {
        dir: &'a str,
    }
    put_json("/roots/current", &Body { dir }).await
}

pub(crate) async fn post_snapshot(enhance_all: bool) -> Result<SnapshotStatus, String> {
    #[derive(Serialize)]
    struct Body {
        enhance_all: bool,
    }
    post_json("/snapshot", &Body { enhance_all }).await
}

pub(crate) async fn get_snapshot_status() -> Result<SnapshotStatus, String> {
    get_json("/snapshot").await
}

pub(crate) async fn post_export_zahir() -> Result<ExportOut, String> {
    post_json("/export/zahir", &serde_json::json!({})).await
}

pub(crate) async fn post_export_lenses() -> Result<ExportOut, String> {
    post_json("/export/lenses", &serde_json::json!({})).await
}
