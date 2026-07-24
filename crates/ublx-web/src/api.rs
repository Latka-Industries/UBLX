//! Same-origin JSON client for `ublx serve`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct DeltaRow {
    pub created_ns: i64,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub delta_type: String,
}

/// Left-pane delta type (API: `added` / `mod` / `removed`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DeltaKind {
    #[default]
    Added,
    Modified,
    Removed,
}

impl DeltaKind {
    pub(crate) const ALL: [Self; 3] = [Self::Added, Self::Modified, Self::Removed];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Modified => "Modified",
            Self::Removed => "Removed",
        }
    }

    pub(crate) fn api_type(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "mod",
            Self::Removed => "removed",
        }
    }

    pub(crate) fn css_class(self) -> &'static str {
        match self {
            Self::Added => "delta-added",
            Self::Modified => "delta-mod",
            Self::Removed => "delta-removed",
        }
    }
}

/// Grouped `/delta` payload for the Delta mode panes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DeltaCatalog {
    pub rows: Vec<DeltaRow>,
}

impl DeltaCatalog {
    pub(crate) fn rows_for(&self, kind: DeltaKind) -> Vec<&DeltaRow> {
        let t = kind.api_type();
        self.rows
            .iter()
            .filter(|r| r.delta_type.eq_ignore_ascii_case(t))
            .collect()
    }

    /// Distinct `created_ns` values, newest first — same idea as TUI snapshot overview.
    pub(crate) fn overview_text(&self) -> String {
        let mut stamps: Vec<i64> = self.rows.iter().map(|r| r.created_ns).collect();
        stamps.sort_unstable_by(|a, b| b.cmp(a));
        stamps.dedup();
        let count = stamps.len();
        let mut lines = vec![
            String::new(),
            format!("{count} snapshot(s) (sorted by time; newest first):"),
            String::new(),
        ];
        for ns in stamps {
            lines.push(format!("  • {}", format_timestamp_ns(ns)));
        }
        lines.join("\n")
    }
}

pub(crate) async fn fetch_delta_catalog() -> DeltaCatalog {
    let rows = get_json::<Vec<DeltaRow>>("/delta")
        .await
        .unwrap_or_default();
    DeltaCatalog { rows }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct EntryRow {
    pub path: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub mtime_ns: Option<i64>,
    #[serde(default)]
    pub zahir: Option<Value>,
    /// Host-parsed Metadata tables (`kv_tables`); present when `?zahir=1`.
    #[serde(default)]
    pub metadata_tables: Option<Vec<SectionView>>,
    /// Host-parsed Writing tables.
    #[serde(default)]
    pub writing_tables: Option<Vec<SectionView>>,
}

/// One Metadata / Writing section from serve (`SectionView` export).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SectionView {
    KeyValue {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        sub_title: bool,
        #[serde(default)]
        rows: Vec<KvRowView>,
    },
    Contents {
        title: String,
        #[serde(default)]
        sub_title: bool,
        #[serde(default)]
        columns: Vec<String>,
        #[serde(default)]
        rows: Vec<Vec<String>>,
    },
    SingleColumnList {
        title: String,
        #[serde(default)]
        values: Vec<String>,
    },
    Tree {
        title: String,
        #[serde(default)]
        roots: Vec<TreeNodeView>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct KvRowView {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct TreeNodeView {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub children: Vec<TreeNodeView>,
    #[serde(default)]
    pub branch: bool,
}

/// Right-pane payload derived from `/entries/{path}?zahir=1` (mirrors TUI section split).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EntryDetail {
    pub path: String,
    pub category: String,
    pub size: u64,
    pub mtime_ns: Option<i64>,
    pub templates: String,
    pub metadata: Option<String>,
    pub writing: Option<String>,
    pub metadata_tables: Vec<SectionView>,
    pub writing_tables: Vec<SectionView>,
}

impl EntryDetail {
    pub(crate) fn from_row(row: EntryRow) -> Self {
        let sections = sections_from_zahir(row.zahir.as_ref());
        Self {
            path: row.path,
            category: row.category,
            size: row.size,
            mtime_ns: row.mtime_ns,
            templates: sections.templates,
            metadata: sections.metadata,
            writing: sections.writing,
            metadata_tables: row.metadata_tables.unwrap_or_default(),
            writing_tables: row.writing_tables.unwrap_or_default(),
        }
    }

    pub(crate) fn has_templates(&self) -> bool {
        !self.templates.is_empty()
    }

    pub(crate) fn has_metadata(&self) -> bool {
        !self.metadata_tables.is_empty() || self.metadata.as_ref().is_some_and(|s| !s.is_empty())
    }

    pub(crate) fn has_writing(&self) -> bool {
        !self.writing_tables.is_empty() || self.writing.as_ref().is_some_and(|s| !s.is_empty())
    }
}

#[derive(Default)]
struct ZahirSections {
    templates: String,
    metadata: Option<String>,
    writing: Option<String>,
}

/// Same key rules as TUI `sectioned_preview_from_zahir` (simplified; no image scrub).
fn sections_from_zahir(zahir: Option<&Value>) -> ZahirSections {
    let Some(value) = zahir else {
        return ZahirSections::default();
    };

    let templates = value
        .get("templates")
        .and_then(|t| serde_json::to_string_pretty(t).ok())
        .filter(|s| !s.is_empty() && s != "null" && s != "[]")
        .unwrap_or_default();

    let metadata = value.as_object().and_then(|obj| {
        let root_file_type = obj.get("file_type");
        let parts: Vec<String> = obj
            .iter()
            .filter(|(k, _)| k.ends_with("_metadata"))
            .filter_map(|(_, v)| {
                let merged = match (root_file_type, v.as_object()) {
                    (Some(ft), Some(meta)) => {
                        let mut m = meta.clone();
                        m.entry("file_type".to_string())
                            .or_insert_with(|| ft.clone());
                        Value::Object(m)
                    }
                    (_, Some(meta)) => Value::Object(meta.clone()),
                    _ => v.clone(),
                };
                serde_json::to_string_pretty(&merged).ok()
            })
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    });

    let writing = value
        .get("writing_footprint")
        .and_then(|w| serde_json::to_string_pretty(w).ok());

    ZahirSections {
        templates,
        metadata,
        writing,
    }
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

/// Fetch one catalog entry with Zahir JSON (`GET /entries/{path}?zahir=1`).
pub(crate) async fn fetch_entry_detail(path: &str) -> Result<EntryDetail, String> {
    let url = format!("/entries/{}?zahir=1", encode_entry_path(path));
    let row = get_json::<EntryRow>(&url).await?;
    Ok(EntryDetail::from_row(row))
}

/// Optional-path wrapper for middle-pane `LocalResource` detail loads.
pub(crate) async fn fetch_entry_detail_opt(path: Option<String>) -> Option<EntryDetail> {
    match path {
        Some(p) => fetch_entry_detail(&p).await.ok(),
        None => None,
    }
}

/// Raw Zahir JSON for clipboard (`GET /entries/{path}?zahir=1`).
pub(crate) async fn fetch_entry_zahir_raw(path: &str) -> Result<Option<Value>, String> {
    let url = format!("/entries/{}?zahir=1", encode_entry_path(path));
    let row = get_json::<EntryRow>(&url).await?;
    Ok(row.zahir)
}

/// Disk file body for Viewer (`GET /content/{path}`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct EntryContent {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub category: String,
    /// `text` or `html`
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub content: String,
    /// PDF page (1-based) when category is PDF.
    #[serde(default)]
    pub page: Option<u32>,
    /// PDF page count when known.
    #[serde(default)]
    pub page_count: Option<u32>,
    /// Explore #12: window start byte.
    #[serde(default)]
    pub offset: Option<u64>,
    /// Explore #12: bytes in this window.
    #[serde(default)]
    pub byte_len: Option<u64>,
    /// Explore #12: requested limit.
    #[serde(default)]
    pub limit: Option<u64>,
    /// Explore #12: file size.
    #[serde(default)]
    pub total_bytes: Option<u64>,
    /// Directory Viewer: nested collapsible tree.
    #[serde(default)]
    pub tree: Option<Vec<TreeNodeView>>,
}

/// Default byte window for large text explore (#12) — matches TUI head/tail chunk.
pub(crate) const CONTENT_WINDOW_BYTES: u64 = 256 * 1024;

/// Threshold where web switches Text/Code from full HTML to windowed plain text.
pub(crate) const CONTENT_WINDOW_MIN_FILE_BYTES: u64 = 512 * 1024;

/// `format`: `Some("html")` | `Some("text")` | `None` (server default).
/// `page`: PDF page for html/raw (1-based).
pub(crate) async fn fetch_entry_content(
    path: &str,
    format: Option<&str>,
) -> Result<EntryContent, String> {
    fetch_entry_content_page(path, format, None).await
}

/// HTML Viewer fetch with optional UBLX theme override for syntect (`?theme=`).
pub(crate) async fn fetch_entry_content_themed(
    path: &str,
    format: Option<&str>,
    theme: Option<String>,
) -> Result<EntryContent, String> {
    fetch_entry_content_query(path, format, None, None, None, theme.as_deref()).await
}

pub(crate) async fn fetch_entry_content_page(
    path: &str,
    format: Option<&str>,
    page: Option<u32>,
) -> Result<EntryContent, String> {
    fetch_entry_content_query(path, format, page, None, None, None).await
}

pub(crate) async fn fetch_entry_content_window(
    path: &str,
    offset: u64,
    limit: u64,
) -> Result<EntryContent, String> {
    fetch_entry_content_query(path, Some("text"), None, Some(offset), Some(limit), None).await
}

async fn fetch_entry_content_query(
    path: &str,
    format: Option<&str>,
    page: Option<u32>,
    offset: Option<u64>,
    limit: Option<u64>,
    theme: Option<&str>,
) -> Result<EntryContent, String> {
    let mut url = format!("/content/{}", encode_entry_path(path));
    let mut q: Vec<String> = Vec::new();
    if let Some(f) = format {
        q.push(format!("format={f}"));
    }
    if let Some(p) = page {
        q.push(format!("page={p}"));
    }
    if let Some(o) = offset {
        q.push(format!("offset={o}"));
    }
    if let Some(l) = limit {
        q.push(format!("limit={l}"));
    }
    if let Some(t) = theme.filter(|s| !s.is_empty()) {
        q.push(format!("theme={}", urlencoding::encode(t)));
    }
    if !q.is_empty() {
        url.push('?');
        url.push_str(&q.join("&"));
    }
    get_json::<EntryContent>(&url).await
}

pub(crate) async fn fetch_lens_names() -> Vec<String> {
    get_json::<Vec<String>>("/lenses").await.unwrap_or_default()
}

/// Members of a named lens (`GET /lenses/{name}`).
pub(crate) async fn fetch_lens_entries(name: &str) -> Vec<EntryRow> {
    let url = format!("/lenses/{}", encode_entry_path(name));
    get_json::<Vec<EntryRow>>(&url).await.unwrap_or_default()
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

pub(crate) async fn fetch_duplicates() -> DuplicatesResponse {
    get_json::<DuplicatesResponse>("/duplicates")
        .await
        .unwrap_or_default()
}

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

pub(crate) async fn put_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    let resp = gloo_net::http::Request::put(url)
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await;
    finish_json(resp).await
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SettingsScope {
    #[default]
    Global,
    Local,
}

impl SettingsScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Local => "local",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Local => "Local",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct SettingsBoolControl {
    pub key: String,
    pub label: String,
    pub value: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct SettingsLayoutControl {
    pub left_pct: u16,
    pub middle_pct: u16,
    pub right_pct: u16,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct ThemeCssBody {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub appearance: String,
    #[serde(default)]
    pub vars: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct SettingsView {
    pub scope: String,
    pub path: String,
    pub exists: bool,
    #[serde(default)]
    pub toml: String,
    #[serde(default)]
    pub bools: Vec<SettingsBoolControl>,
    #[serde(default)]
    pub layout: SettingsLayoutControl,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub themes: Vec<String>,
    #[serde(default)]
    pub theme_picker: Vec<ThemePickerRow>,
    #[serde(default)]
    pub bg_opacity: f32,
    /// `none` | `abbrev` | `full`
    #[serde(default = "default_typed_column_tables")]
    pub typed_column_tables: String,
    #[serde(default)]
    pub css: ThemeCssBody,
}

/// Command Mode theme picker rows (`Dark` / `Light` sections + swatches).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ThemePickerRow {
    Section {
        label: String,
    },
    Theme {
        name: String,
        #[serde(default)]
        appearance: String,
        /// HSL components (`"H S% L%"`).
        #[serde(default)]
        swatch: String,
        /// Full tokens for live highlight preview.
        #[serde(default)]
        css: ThemeCssBody,
    },
}

impl ThemePickerRow {
    pub(crate) fn theme_name(&self) -> Option<&str> {
        match self {
            Self::Theme { name, .. } => Some(name.as_str()),
            Self::Section { .. } => None,
        }
    }

    pub(crate) fn theme_css(&self) -> Option<&ThemeCssBody> {
        match self {
            Self::Theme { css, .. } if !css.vars.is_empty() || !css.name.is_empty() => Some(css),
            _ => None,
        }
    }

    pub(crate) fn index_of_theme(rows: &[Self], current: &str) -> usize {
        rows.iter()
            .filter_map(Self::theme_name)
            .position(|n| n == current)
            .unwrap_or(0)
    }

    pub(crate) fn theme_name_at(rows: &[Self], index: usize) -> Option<&str> {
        rows.iter().filter_map(Self::theme_name).nth(index)
    }

    pub(crate) fn theme_css_at(rows: &[Self], index: usize) -> Option<&ThemeCssBody> {
        rows.iter().filter_map(Self::theme_css).nth(index)
    }
}

fn default_typed_column_tables() -> String {
    "abbrev".into()
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct SettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_hidden_files: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_enhance_all: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_enhance_on_new_root: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_snapshot_on_startup: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_opacity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<SettingsLayoutPatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed_column_tables: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SettingsLayoutPatch {
    pub left_pct: u16,
    pub middle_pct: u16,
    pub right_pct: u16,
}

pub(crate) async fn fetch_settings(scope: SettingsScope) -> Result<SettingsView, String> {
    get_json::<SettingsView>(&format!("/settings/{}", scope.as_str())).await
}

pub(crate) async fn patch_settings(
    scope: SettingsScope,
    patch: &SettingsPatch,
) -> Result<SettingsView, String> {
    patch_json(&format!("/settings/{}", scope.as_str()), patch).await
}

/// Encode a catalog-relative path for `/entries/{*path}` (preserve `/`, encode each segment).
pub(crate) fn encode_entry_path(path: &str) -> String {
    path.split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) async fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    finish_json(gloo_net::http::Request::get(url).send().await).await
}

pub(crate) async fn patch_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    let resp = gloo_net::http::Request::patch(url)
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await;
    finish_json(resp).await
}

pub(crate) async fn post_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    let resp = gloo_net::http::Request::post(url)
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await;
    finish_json(resp).await
}

pub(crate) async fn delete_json<T: for<'de> Deserialize<'de>, B: Serialize>(
    url: &str,
    body: &B,
) -> Result<T, String> {
    let resp = gloo_net::http::Request::delete(url)
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await;
    finish_json(resp).await
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RenameBody {
    pub path: String,
    pub new_name: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BulkRenameItem {
    pub path: String,
    pub new_name: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BulkRenameBody {
    pub renames: Vec<BulkRenameItem>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PathsBody {
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct EnhancePolicyBody {
    pub path: String,
    pub policy: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CreateLensBody {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RenameLensBody {
    pub new_name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RenameOut {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct BulkOpOut {
    #[serde(default)]
    pub renamed: Option<usize>,
    #[serde(default)]
    pub deleted: Option<usize>,
    #[serde(default)]
    pub failed: Option<serde_json::Value>,
}

pub(crate) async fn api_rename(path: &str, new_name: &str) -> Result<String, String> {
    let out: RenameOut = post_json(
        "/fs/rename",
        &RenameBody {
            path: path.into(),
            new_name: new_name.into(),
        },
    )
    .await?;
    Ok(out.path)
}

pub(crate) async fn api_bulk_rename(renames: Vec<BulkRenameItem>) -> Result<String, String> {
    let out: BulkOpOut = post_json("/fs/bulk-rename", &BulkRenameBody { renames }).await?;
    let n = out.renamed.unwrap_or(0);
    if let Some(f) = out.failed {
        return Err(format!("renamed {n}; then failed: {f}"));
    }
    Ok(format!("Renamed {n}"))
}

pub(crate) async fn api_delete(paths: Vec<String>) -> Result<String, String> {
    let out: BulkOpOut = post_json("/fs/delete", &PathsBody { paths }).await?;
    let n = out.deleted.unwrap_or(0);
    if let Some(f) = out.failed {
        return Err(format!("deleted {n}; then failed: {f}"));
    }
    Ok(format!("Deleted {n}"))
}

pub(crate) async fn api_enhance(paths: Vec<String>) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        enhanced: usize,
        failed: usize,
    }
    let out: Out = post_json("/fs/enhance", &PathsBody { paths }).await?;
    Ok(format!("Enhanced {}; failed {}", out.enhanced, out.failed))
}

pub(crate) async fn api_enhance_policy(path: &str, policy: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        policy: String,
    }
    let out: Out = post_json(
        "/fs/enhance-policy",
        &EnhancePolicyBody {
            path: path.into(),
            policy: policy.into(),
        },
    )
    .await?;
    Ok(format!("Enhance policy: {}", out.policy))
}

pub(crate) async fn api_create_lens(name: &str, paths: Vec<String>) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        name: String,
        added: usize,
    }
    let out: Out = post_json(
        "/lenses",
        &CreateLensBody {
            name: name.into(),
            paths,
        },
    )
    .await?;
    Ok(format!("Lens {} (+{})", out.name, out.added))
}

pub(crate) async fn api_rename_lens(old: &str, new_name: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        name: String,
    }
    let url = format!("/lenses/{}", urlencoding::encode(old));
    let out: Out = patch_json(
        &url,
        &RenameLensBody {
            new_name: new_name.into(),
        },
    )
    .await?;
    Ok(format!("Lens renamed to {}", out.name))
}

pub(crate) async fn api_delete_lens(name: &str) -> Result<(), String> {
    let url = format!("/lenses/{}", urlencoding::encode(name));
    let resp = gloo_net::http::Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(http_error_message(resp).await);
    }
    Ok(())
}

pub(crate) async fn api_add_to_lens(lens: &str, paths: Vec<String>) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        count: usize,
    }
    let url = format!("/lenses/{}/paths", urlencoding::encode(lens));
    let out: Out = post_json(&url, &PathsBody { paths }).await?;
    Ok(format!("Added {} to {lens}", out.count))
}

pub(crate) async fn api_remove_from_lens(lens: &str, paths: Vec<String>) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Out {
        count: usize,
    }
    let url = format!("/lenses/{}/paths", urlencoding::encode(lens));
    let out: Out = delete_json(&url, &PathsBody { paths }).await?;
    Ok(format!("Removed {} from {lens}", out.count))
}

async fn finish_json<T: for<'de> Deserialize<'de>>(
    resp: Result<gloo_net::http::Response, gloo_net::Error>,
) -> Result<T, String> {
    let resp = resp.map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(http_error_message(resp).await);
    }
    resp.json::<T>().await.map_err(|e| e.to_string())
}

async fn http_error_message(resp: gloo_net::http::Response) -> String {
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if let Ok(v) = serde_json::from_str::<Value>(&text)
        && let Some(err) = v.get("error").and_then(|e| e.as_str())
    {
        return format!("{status}: {err}");
    }
    if text.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("{status}: {text}")
    }
}

/// Match TUI `format_bytes` (1024-based).
pub(crate) fn format_bytes(n: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    if n < 1024 {
        format!("{n} B")
    } else if (n as f64) < MIB {
        format!("{:.2} KB", n as f64 / KIB)
    } else if (n as f64) < GIB {
        format!("{:.2} MB", n as f64 / MIB)
    } else {
        format!("{:.2} GB", n as f64 / GIB)
    }
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
