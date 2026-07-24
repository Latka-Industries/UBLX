//! Background rendering for large viewer bodies (markdown, syntect, delimiter tables) so the TUI
//! thread does not block on first paint. Uses `std::thread` + `std::sync::mpsc` (no Tokio).

use std::sync::Arc;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

use ratatui::text::Text;

use crate::engine::{cache, viewer_async};
use crate::layout::setup::{RightPaneContent, UblxState};
use crate::render::viewers::{csv_handler, markdown, syntect_text};
use crate::themes;

// --- Shared builders (sync path and background worker) -----------------------------------------

/// Markdown viewer cache entry. Pass `content_identity` from the **UI thread** so it matches the
/// live preview’s [`ViewerContentIdentity`] (mtime + len, or len-only).
#[must_use]
pub fn build_markdown_cache_entry(
    path: &str,
    raw: &str,
    content_width: u16,
    theme_key: String,
    content_identity: cache::ViewerContentIdentity,
) -> cache::ViewerTextCacheEntry {
    let doc = markdown::parse_markdown(raw);
    let text = doc.to_text(content_width);
    let line_count = text.lines.len();
    cache::ViewerTextCacheEntry {
        path: path.to_string(),
        content_width,
        theme_name: theme_key,
        content_identity,
        line_count,
        text,
        syntect: None,
    }
}

fn syntect_cache_entry_from_text(
    p: &cache::CodeViewerCacheParams<'_>,
    text: Text<'static>,
    content_identity: cache::ViewerContentIdentity,
) -> cache::ViewerTextCacheEntry {
    let line_count = text.lines.len();
    cache::ViewerTextCacheEntry {
        path: p.path.to_string(),
        content_width: p.content_width,
        theme_name: p.theme_name.to_string(),
        content_identity,
        line_count,
        text,
        syntect: Some((p.appearance, p.category)),
    }
}

#[must_use]
pub fn build_syntect_cache_entry(
    p: &cache::CodeViewerCacheParams<'_>,
    content_identity: cache::ViewerContentIdentity,
) -> cache::ViewerTextCacheEntry {
    let text = syntect_text::highlight_viewer_for_palette(
        p.raw,
        p.path,
        p.category,
        themes::get(Some(p.theme_name)),
    );
    syntect_cache_entry_from_text(p, text, content_identity)
}

#[must_use]
pub fn build_csv_cache_entry(
    path: &str,
    raw: &str,
    content_width: u16,
    theme_key: String,
    content_identity: cache::ViewerContentIdentity,
) -> Option<cache::ViewerTextCacheEntry> {
    let rows = csv_handler::parse_csv(raw, Some(path)).ok()?;
    if rows.is_empty() {
        return None;
    }
    let (text, line_count) = if csv_handler::should_render_as_table(&rows) {
        let (table_string, line_count) =
            csv_handler::table_string_and_line_count(&rows, content_width);
        let text = csv_handler::table_string_to_text(&table_string);
        debug_assert_eq!(line_count, text.lines.len());
        (text, line_count)
    } else {
        let total_rows_hint = csv_handler::total_rows_hint_from_raw(raw);
        let text = csv_handler::wide_structured_text(&rows, content_width, total_rows_hint);
        let line_count = text.lines.len();
        (text, line_count)
    };
    Some(cache::ViewerTextCacheEntry {
        path: path.to_string(),
        content_width,
        theme_name: theme_key,
        content_identity,
        line_count,
        text,
        syntect: None,
    })
}

/// Stable key for matching completed work to the current selection and layout
fn job_key_for(
    rc: &RightPaneContent,
    content_width: u16,
    kind: viewer_async::ViewerAsyncJobKind,
) -> Option<viewer_async::ViewerAsyncJobKey> {
    let path = rc.snap_meta.path.as_deref()?.to_string();
    Some(viewer_async::ViewerAsyncJobKey {
        path,
        content_width,
        theme_name: themes::current().name.to_string(),
        kind,
    })
}

fn spawn_job(
    tx: mpsc::Sender<viewer_async::ViewerAsyncDone>,
    done_key: viewer_async::ViewerAsyncJobKey,
    work: impl FnOnce() -> viewer_async::ViewerAsyncResult + Send + 'static,
) {
    let _ = thread::Builder::new()
        .name("ublx-viewer-async".into())
        .spawn(move || {
            let result = work();
            let _ = tx.send(viewer_async::ViewerAsyncDone {
                key: done_key,
                result,
            });
        });
}

/// Worker threads start with no palette in TLS; install it before any code that calls [`themes::current`].
fn sync_worker_palette(theme_name: &str) {
    themes::set_current(Some(theme_name));
}

/// Clear prior async state, open a channel, and return the sender + job key; [`None`] if we should
/// skip (no path, or identical job already pending).
fn begin_viewer_async_job(
    state: &mut UblxState,
    rc: &RightPaneContent,
    content_width: u16,
    kind: viewer_async::ViewerAsyncJobKind,
) -> Option<(
    mpsc::Sender<viewer_async::ViewerAsyncDone>,
    viewer_async::ViewerAsyncJobKey,
)> {
    let key = job_key_for(rc, content_width, kind)?;
    if state.viewer_async.pending_key.as_ref() == Some(&key) && state.viewer_async.rx.is_some() {
        return None;
    }
    state.viewer_text_cache = None;
    state.viewer_async.clear();
    let (tx, rx) = mpsc::channel();
    state.viewer_async.rx = Some(rx);
    state.viewer_async.pending_key = Some(key.clone());
    Some((tx, key))
}

/// Apply one completed viewer-async message if any.
///
/// Completion is matched against [`ViewerAsyncState::pending_key`] only — not against a trial
/// layout width. Width convergence in the right pane uses several candidate `text_w` values per
/// frame; comparing `job_key_for(.., trial_width, ..)` to `done.key` discarded valid messages and
/// cleared the channel, which caused “Loading…” forever and flicker vs sync highlight.
pub fn poll_viewer_async(state: &mut UblxState, rc: &RightPaneContent) {
    let recv = match state.viewer_async.rx.as_ref() {
        Some(rx) => rx.try_recv(),
        None => return,
    };
    match recv {
        Ok(done) => {
            if state.viewer_async.pending_key.as_ref() != Some(&done.key) {
                state.viewer_async.rx = None;
                state.viewer_async.pending_key = None;
                return;
            }
            let path_ok = rc
                .snap_meta
                .path
                .as_deref()
                .is_some_and(|p| p == done.key.path);
            if !path_ok {
                state.viewer_async.rx = None;
                state.viewer_async.pending_key = None;
                return;
            }
            match done.result {
                viewer_async::ViewerAsyncResult::Markdown(e)
                | viewer_async::ViewerAsyncResult::Code(e) => {
                    state.viewer_text_cache = Some(e);
                    state.viewer_async.pending_key = None;
                    state.viewer_async.rx = None;
                }
                viewer_async::ViewerAsyncResult::Csv(key, Some(e)) => {
                    state.csv_table_text_lru.insert(key, e);
                    state.viewer_async.pending_key = None;
                    state.viewer_async.rx = None;
                }
                viewer_async::ViewerAsyncResult::Csv(_, None) => {
                    state.viewer_async.pending_key = None;
                    state.viewer_async.rx = None;
                }
            }
        }
        Err(TryRecvError::Empty) => {}
        Err(TryRecvError::Disconnected) => {
            state.viewer_async.rx = None;
            state.viewer_async.pending_key = None;
        }
    }
}

pub fn reset_viewer_async(state: &mut UblxState) {
    state.viewer_async.clear();
}

pub fn schedule_markdown(
    state: &mut UblxState,
    rc: &RightPaneContent,
    content_width: u16,
    path: &str,
    raw: Arc<str>,
    theme_key: String,
) {
    let Some((tx, done_key)) = begin_viewer_async_job(
        state,
        rc,
        content_width,
        viewer_async::ViewerAsyncJobKind::Markdown,
    ) else {
        return;
    };
    let content_identity = cache::viewer_content_identity(raw.as_ref(), rc.snap_meta.mtime_ns);
    let path = path.to_string();
    spawn_job(tx, done_key, move || {
        sync_worker_palette(theme_key.as_str());
        viewer_async::ViewerAsyncResult::Markdown(build_markdown_cache_entry(
            &path,
            raw.as_ref(),
            content_width,
            theme_key,
            content_identity,
        ))
    });
}

pub fn schedule_syntect(
    state: &mut UblxState,
    rc: &RightPaneContent,
    content_width: u16,
    path: &str,
    raw: Arc<str>,
    theme_name: String,
) {
    let Some((tx, done_key)) = begin_viewer_async_job(
        state,
        rc,
        content_width,
        viewer_async::ViewerAsyncJobKind::Code,
    ) else {
        return;
    };
    let content_identity = cache::viewer_content_identity(raw.as_ref(), rc.snap_meta.mtime_ns);
    let path_owned = path.to_string();
    let appearance = themes::current().appearance;
    let category = rc.ublx_db_category();
    let mtime_ns = rc.snap_meta.mtime_ns;
    spawn_job(tx, done_key, move || {
        let p = cache::CodeViewerCacheParams {
            path: path_owned.as_str(),
            raw: raw.as_ref(),
            content_width,
            theme_name: theme_name.as_str(),
            appearance,
            category,
            mtime_ns,
        };
        viewer_async::ViewerAsyncResult::Code(build_syntect_cache_entry(&p, content_identity))
    });
}

pub fn schedule_csv(
    state: &mut UblxState,
    rc: &RightPaneContent,
    content_width: u16,
    path: &str,
    raw: Arc<str>,
    theme_key: String,
    table_key: cache::ViewerTableCacheKey,
) {
    let Some((tx, done_key)) = begin_viewer_async_job(
        state,
        rc,
        content_width,
        viewer_async::ViewerAsyncJobKind::Csv,
    ) else {
        return;
    };
    let path = path.to_string();
    let identity = table_key.identity.clone();
    let cache_key = table_key;
    spawn_job(tx, done_key, move || {
        sync_worker_palette(theme_key.as_str());
        let entry = build_csv_cache_entry(&path, raw.as_ref(), content_width, theme_key, identity);
        viewer_async::ViewerAsyncResult::Csv(cache_key, entry)
    });
}

/// True while a large-file async job is still **waiting for first useful cache** (loading line).
///
/// Uses [`ViewerAsyncJobKey::content_width`] from the pending job — not a trial width from the
/// width-convergence loop — so the UI does not alternate between sync highlight and “Loading…”.
#[must_use]
pub fn viewer_async_placeholder_active(state: &UblxState, rc: &RightPaneContent) -> bool {
    let Some(pending) = state.viewer_async.pending_key.as_ref() else {
        return false;
    };
    if state.viewer_async.rx.is_none() {
        return false;
    }
    let Some(path) = rc.snap_meta.path.as_deref() else {
        return false;
    };
    if path != pending.path.as_str() || themes::current().name != pending.theme_name {
        return false;
    }
    let w = pending.content_width;
    let theme = themes::current().name;
    let raw = rc.viewer.as_deref().unwrap_or("");
    match pending.kind {
        viewer_async::ViewerAsyncJobKind::Code => {
            let p = cache::CodeViewerCacheParams {
                path,
                raw,
                content_width: w,
                theme_name: theme,
                appearance: themes::current().appearance,
                category: rc.ublx_db_category(),
                mtime_ns: rc.snap_meta.mtime_ns,
            };
            !state
                .viewer_text_cache
                .as_ref()
                .is_some_and(|e| e.matches_syntect_viewer(&p))
        }
        viewer_async::ViewerAsyncJobKind::Markdown => !state
            .viewer_text_cache
            .as_ref()
            .is_some_and(|e| e.matches_markdown_viewer(path, w, theme, raw)),
        viewer_async::ViewerAsyncJobKind::Csv => {
            let key = cache::viewer_table_cache_key(path, w, theme, raw, rc.snap_meta.mtime_ns);
            !state
                .csv_table_text_lru
                .entries
                .iter()
                .any(|(k, _)| k == &key)
        }
    }
}
