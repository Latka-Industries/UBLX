//! Disk file body (`/content/{path}`) fetch helpers.

use serde::Deserialize;

use super::entries::TreeNodeView;
use super::http::{encode_entry_path, get_json};

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
