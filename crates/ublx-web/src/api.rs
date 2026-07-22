//! Same-origin JSON client for `ublx serve`.

use serde::Deserialize;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CatalogFlags {
    pub has_lenses: bool,
    pub has_delta: bool,
    /// No serve API yet — always false until duplicates routes exist.
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

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct DeltaRow {
    pub created_ns: i64,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub delta_type: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct EntryRow {
    pub path: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub size: u64,
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
    let delta = get_json::<Vec<DeltaRow>>("/delta")
        .await
        .unwrap_or_default();
    let last_snapshot_ns = delta.iter().map(|r| r.created_ns).max();
    let has_delta = !delta.is_empty();
    CatalogFlags {
        has_lenses,
        has_delta,
        has_duplicates: false,
        health,
        root,
        last_snapshot_ns,
    }
}

pub(crate) async fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<T>()
        .await
        .map_err(|e| e.to_string())
}

/// Match TUI `format_timestamp_ns`: local `YYYY-MM-DD HH:MM:SS`.
pub(crate) fn format_timestamp_ns(ns: i64) -> String {
    let ms = (ns as f64) / 1_000_000.0;
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms));
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.get_full_year() as i32,
        date.get_month() + 1,
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds(),
    )
}
