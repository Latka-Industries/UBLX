//! Small pure helpers + clipboard / browser open (no `SpaceMenuCtx` — avoids module cycles).

use leptos::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::api::encode_entry_path;
use crate::multiselect::MultiselectCtx;

pub(super) fn basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

pub(super) fn bulk_paths(ms: MultiselectCtx) -> Vec<String> {
    let mut v: Vec<String> = ms.selected.get_untracked().into_iter().collect();
    v.sort();
    v
}

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

pub(super) async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let _ = web_sys::window().map(|w| {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        });
    });
    let _ = JsFuture::from(promise).await;
}
