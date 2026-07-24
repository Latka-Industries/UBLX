//! Shell catalog flags, lenses list, and duplicates.

use serde::Deserialize;

use super::delta::DeltaRow;
use super::entries::EntryRow;
use super::http::{get_json, lens_url};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CatalogFlags {
    pub has_lenses: bool,
    pub has_delta: bool,
    pub has_duplicates: bool,
    pub health: Option<HealthBody>,
    pub root: Option<String>,
    /// Latest `delta_log.created_ns` (same source as TUI status “Last Snapshot”).
    pub last_snapshot_ns: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct HealthBody {
    pub ok: bool,
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub uptime_secs: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct CurrentRoot {
    pub path: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct DuplicateGroupRow {
    pub id: usize,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct DuplicatesResponse {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub groups: Vec<DuplicateGroupRow>,
}

pub(crate) async fn load_catalog_flags() -> CatalogFlags {
    let health = get_json::<HealthBody>("/health").await.ok();
    let root = get_json::<CurrentRoot>("/roots/current")
        .await
        .ok()
        .map(|r| r.path);
    let has_lenses = get_json::<Vec<String>>("/lenses")
        .await
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let has_duplicates = get_json::<DuplicatesResponse>("/duplicates")
        .await
        .map(|d| !d.groups.is_empty())
        .unwrap_or(false);
    let delta = get_json::<Vec<DeltaRow>>("/delta")
        .await
        .unwrap_or_default();
    let last_snapshot_ns = delta.iter().map(|r| r.created_ns).max();
    let has_delta = !delta.is_empty();
    CatalogFlags {
        has_lenses,
        has_delta,
        has_duplicates,
        health,
        root,
        last_snapshot_ns,
    }
}

pub(crate) async fn fetch_lens_names() -> Vec<String> {
    get_json::<Vec<String>>("/lenses").await.unwrap_or_default()
}

/// Members of a named lens (`GET /lenses/{name}`).
pub(crate) async fn fetch_lens_entries(name: &str) -> Vec<EntryRow> {
    get_json::<Vec<EntryRow>>(&lens_url(name))
        .await
        .unwrap_or_default()
}

pub(crate) async fn fetch_duplicates() -> DuplicatesResponse {
    get_json::<DuplicatesResponse>("/duplicates")
        .await
        .unwrap_or_default()
}
