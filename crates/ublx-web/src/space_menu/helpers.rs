//! Small pure helpers + clipboard / browser open (no `SpaceMenuCtx` — avoids module cycles).

use leptos::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::api::encode_entry_path;
use crate::multiselect::MultiselectCtx;

pub(super) use crate::util::sleep_ms;

/// Last path segment (basename).
pub(super) fn basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Sorted multi-select paths for bulk menus.
pub(super) fn bulk_paths(ms: MultiselectCtx) -> Vec<String> {
    let mut v: Vec<String> = ms.selected.get_untracked().into_iter().collect();
    v.sort();
    v
}

/// Join catalog root + relative path for Copy Path / display.
pub(super) fn absolute_path(root: Option<&str>, rel: &str) -> String {
    match root {
        Some(r) if !r.is_empty() => {
            let r = r.trim_end_matches('/');
            if rel.is_empty() {
                r.to_string()
            } else {
                format!("{r}/{rel}")
            }
        }
        _ => rel.to_string(),
    }
}

/// Open `/content/…?format=raw` in a new tab (images only at call sites).
pub(super) fn open_in_browser(path: &str) {
    let url = format!("/content/{}?format=raw", encode_entry_path(path));
    if let Some(w) = web_sys::window() {
        let _ = w.open_with_url_and_target(&url, "_blank");
    }
}

pub(super) async fn write_clipboard(text: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let clipboard = window.navigator().clipboard();
    let promise = clipboard.write_text(text);
    JsFuture::from(promise)
        .await
        .map(|_| ())
        .map_err(|_| "clipboard write failed".to_string())
}
