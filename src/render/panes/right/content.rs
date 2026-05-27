//! Right-pane scroll **content**: [`content_display_text`] for every tab, plus **Viewer**-tab helpers
//! (CSV / markdown / raster labels, [`ensure_viewer_text_cache`], line counts, wrap).

use ratatui::style::Modifier;
use ratatui::text::{Line, Span, Text};

use crate::engine::cache::{self, ViewerTextCacheEntry};
use crate::engine::db_ops::UblxDbCategory;
use crate::integrations::{ZahirFT, delimiter_from_path_for_viewer};
use crate::layout::{
    setup::{RightPaneContent, RightPaneMode, UblxState},
    style,
};
use crate::render::{kv_tables, viewers};
use crate::themes;
use crate::ui::UI_STRINGS;

/// Bundles snapshot path/raw/theme with appearance + file identity for syntect cache match/build.
fn syntect_viewer_cache_params<'a>(
    path: &'a str,
    raw: &'a str,
    content_width: u16,
    theme_name: &'a str,
    rc: &RightPaneContent,
) -> cache::CodeViewerCacheParams<'a> {
    cache::CodeViewerCacheParams {
        path,
        raw,
        content_width,
        theme_name,
        appearance: themes::current().appearance,
        category: rc.ublx_db_category(),
        mtime_ns: rc.snap_meta.mtime_ns,
    }
}

#[inline]
fn viewer_is_csv(rc: &RightPaneContent) -> bool {
    rc.zahir_file_type() == Some(ZahirFT::Csv)
}

#[inline]
fn viewer_show_delimited_table(rc: &RightPaneContent) -> bool {
    if viewer_is_markdown(rc) {
        return false;
    }
    if viewer_is_csv(rc) {
        return true;
    }
    rc.snap_meta
        .path
        .as_deref()
        .is_some_and(|p| delimiter_from_path_for_viewer(p).is_some())
}

#[inline]
fn viewer_is_markdown(rc: &RightPaneContent) -> bool {
    rc.zahir_file_type() == Some(ZahirFT::Markdown)
}

/// Code only when the snapshot [`UblxDbCategory`] (DB `category` column) is a zahir type we highlight.
#[inline]
fn viewer_uses_syntect_highlight(rc: &RightPaneContent) -> bool {
    rc.snap_meta.path.is_some()
        && matches!(
            rc.ublx_db_category(),
            UblxDbCategory::Zahir(
                ZahirFT::Json
                    | ZahirFT::Toml
                    | ZahirFT::Yaml
                    | ZahirFT::Xml
                    | ZahirFT::Html
                    | ZahirFT::Ini
                    | ZahirFT::Log
                    | ZahirFT::Code
            )
        )
}

fn reset_viewer_cache_and_async(state: &mut UblxState) {
    state.viewer_text_cache = None;
    state.viewer_preview_source = None;
    viewers::async_tools::reset_viewer_async(state);
}

fn sync_viewer_cache_for_content_identity(
    state: &mut UblxState,
    path: &str,
    content_id: cache::ViewerContentIdentity,
) {
    let source_matches = state
        .viewer_preview_source
        .as_ref()
        .is_some_and(|(p, id)| p.as_str() == path && *id == content_id);
    if !source_matches {
        state.viewer_text_cache = None;
        viewers::async_tools::reset_viewer_async(state);
        state
            .csv_table_text_lru
            .retain_keys(|k| k.path == path && k.identity == content_id);
        state.viewer_preview_source = Some((path.to_string(), content_id));
    }
}

/// Delimited (CSV / TSV) path: update cache or schedule work. Returns `true` if [`ensure_viewer_text_cache`] should return.
fn delimited_table_viewer_cache_or_return(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
    path: &str,
    raw: &str,
    raw_arc: &std::sync::Arc<str>,
    theme_name: &str,
) -> bool {
    if !viewer_show_delimited_table(right_content) {
        return false;
    }
    state.viewer_text_cache = None;
    let key = cache::viewer_table_cache_key(
        path,
        content_width,
        theme_name,
        raw,
        right_content.snap_meta.mtime_ns,
    );
    if state.csv_table_text_lru.get(&key).is_some() {
        return true;
    }
    if raw.len() >= cache::VIEWER_TEXT_CACHE.min_csv_bytes {
        viewers::async_tools::schedule_csv(
            state,
            right_content,
            content_width,
            path,
            raw_arc.clone(),
            theme_name.to_string(),
            key,
        );
        return true;
    }
    let Some(entry) = viewers::async_tools::build_csv_cache_entry(
        path,
        raw,
        content_width,
        theme_name.to_string(),
        key.identity.clone(),
    ) else {
        return true;
    };
    state.csv_table_text_lru.insert(key, entry);
    true
}

/// Run once after width convergence to poll + schedule async viewer work (CSV / markdown / syntect).
pub fn ensure_viewer_text_cache(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
) {
    viewers::async_tools::poll_viewer_async(state, right_content);

    if state.right_pane_mode != RightPaneMode::Viewer {
        reset_viewer_cache_and_async(state);
        return;
    }
    let Some(path) = right_content.snap_meta.path.as_deref() else {
        reset_viewer_cache_and_async(state);
        return;
    };
    let Some(raw_arc) = right_content.viewer.clone() else {
        reset_viewer_cache_and_async(state);
        return;
    };
    let raw = raw_arc.as_ref();
    let content_id = cache::viewer_content_identity(raw, right_content.snap_meta.mtime_ns);
    sync_viewer_cache_for_content_identity(state, path, content_id);
    let palette = themes::current();
    let theme_key = palette.name.to_string();
    let theme = palette.name;

    if delimited_table_viewer_cache_or_return(
        state,
        right_content,
        content_width,
        path,
        raw,
        &raw_arc,
        theme,
    ) {
        return;
    }

    if viewer_is_markdown(right_content) && raw.len() >= cache::VIEWER_TEXT_CACHE.min_markdown_bytes
    {
        if let Some(ref e) = state.viewer_text_cache
            && e.matches_markdown_viewer(path, content_width, theme, raw)
        {
            return;
        }
        viewers::async_tools::schedule_markdown(
            state,
            right_content,
            content_width,
            path,
            raw_arc.clone(),
            theme_key,
        );
        return;
    }

    if viewer_uses_syntect_highlight(right_content)
        && raw.len() >= cache::VIEWER_TEXT_CACHE.min_syntect_bytes
    {
        let p = syntect_viewer_cache_params(path, raw, content_width, theme, right_content);
        if let Some(ref e) = state.viewer_text_cache
            && e.matches_syntect_viewer(&p)
        {
            return;
        }
        viewers::async_tools::schedule_syntect(
            state,
            right_content,
            content_width,
            path,
            raw_arc.clone(),
            theme_key,
        );
        return;
    }

    state.viewer_text_cache = None;
}

fn csv_cached_entry<'a>(
    state: &'a mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
    theme: &str,
) -> Option<&'a ViewerTextCacheEntry> {
    let path = right_content.snap_meta.path.as_deref()?;
    let raw = right_content.viewer.as_deref()?;
    let key = cache::viewer_table_cache_key(
        path,
        content_width,
        theme,
        raw,
        right_content.snap_meta.mtime_ns,
    );
    state.csv_table_text_lru.get(&key)
}

/// True if the active text viewport matches the current viewer cache (async placeholder or cached entry).
pub fn viewer_text_cache_viewport_active(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
) -> bool {
    let Some(vp) = right_content.snap_meta.path.as_deref() else {
        return false;
    };
    let Some(raw) = right_content.viewer.as_deref() else {
        return false;
    };
    let theme = themes::current().name;
    if viewer_show_delimited_table(right_content) {
        if raw.len() >= cache::VIEWER_TEXT_CACHE.min_csv_bytes
            && viewers::async_tools::viewer_async_placeholder_active(state, right_content)
        {
            return false;
        }
        return csv_cached_entry(state, right_content, content_width, theme).is_some();
    }
    state
        .viewer_text_cache
        .as_ref()
        .is_some_and(|e: &ViewerTextCacheEntry| {
            (viewer_is_markdown(right_content)
                && raw.len() >= cache::VIEWER_TEXT_CACHE.min_markdown_bytes
                && e.matches_markdown_viewer(vp, content_width, theme, raw))
                || (viewer_uses_syntect_highlight(right_content)
                    && raw.len() >= cache::VIEWER_TEXT_CACHE.min_syntect_bytes
                    && e.matches_syntect_viewer(&syntect_viewer_cache_params(
                        vp,
                        raw,
                        content_width,
                        theme,
                        right_content,
                    )))
        })
}

/// `scroll_viewport_h` — height of the scrollable content area (e.g. padded body height). When
/// [`viewer_async_placeholder_active`] is true and this is `Some`, we return a line count **above**
/// the viewport so [`scrollable_content::viewport_text_width`] reserves the scrollbar column. If we
/// instead return `1` while loading, width converges to “full row”; after async completes the real
/// line count usually needs a scrollbar → width shrinks by 1 → cache key / `content_width` no longer
/// matches → reschedule and visible flicker.
pub fn viewer_total_lines(
    right_content: &RightPaneContent,
    content_width: u16,
    use_kv_tables: Option<&str>,
    state: &mut UblxState,
    scroll_viewport_h: Option<u16>,
) -> usize {
    match (state.right_pane_mode, use_kv_tables) {
        (_, Some(json)) => {
            kv_tables::content_height(json, state.panels.column_stats) as usize
        }
        (RightPaneMode::Viewer, _) => {
            viewer_total_lines_for_viewer(right_content, content_width, state, scroll_viewport_h)
        }
        (RightPaneMode::Templates, _) => right_content.templates.lines().count(),
        (RightPaneMode::Writing | RightPaneMode::Metadata, _) => 0,
    }
}

fn viewer_total_lines_for_viewer(
    right_content: &RightPaneContent,
    content_width: u16,
    state: &mut UblxState,
    scroll_viewport_h: Option<u16>,
) -> usize {
    if viewers::async_tools::viewer_async_placeholder_active(state, right_content) {
        return scroll_viewport_h.map_or(1, |h| (h as usize).saturating_add(1));
    }
    if let Some(lines) = cached_viewer_line_count(state, right_content, content_width) {
        return lines;
    }
    if let Some(lines) = delimited_line_count_uncached(right_content, content_width) {
        return lines;
    }
    if viewers::images::is_raster_preview_category(right_content) {
        return raster_viewer_line_count(state, right_content, content_width);
    }
    if viewer_is_markdown(right_content) {
        let raw = right_content.viewer.as_deref().unwrap_or("");
        return viewers::markdown::parse_markdown(raw)
            .to_text(content_width)
            .lines
            .len();
    }
    if viewer_uses_syntect_highlight(right_content) {
        return right_content.viewer.as_deref().map_or(0, |t| {
            let n = t.lines().count();
            if n > 1 {
                n
            } else {
                wrapped_line_count(t, content_width) as usize
            }
        });
    }
    tree_with_policy_line_count(right_content, content_width)
}

fn cached_viewer_line_count(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
) -> Option<usize> {
    let theme = themes::current().name;
    if viewer_show_delimited_table(right_content) {
        return csv_cached_entry(state, right_content, content_width, theme).map(|e| e.line_count);
    }
    let (Some(vp), Some(raw), Some(e)) = (
        right_content.snap_meta.path.as_deref(),
        right_content.viewer.as_deref(),
        state.viewer_text_cache.as_ref(),
    ) else {
        return None;
    };
    let syntect_args = syntect_viewer_cache_params(vp, raw, content_width, theme, right_content);
    let cached = if viewer_is_markdown(right_content)
        && raw.len() >= cache::VIEWER_TEXT_CACHE.min_markdown_bytes
    {
        e.matches_markdown_viewer(vp, content_width, theme, raw)
    } else if viewer_uses_syntect_highlight(right_content)
        && raw.len() >= cache::VIEWER_TEXT_CACHE.min_syntect_bytes
    {
        e.matches_syntect_viewer(&syntect_args)
    } else {
        false
    };
    cached.then_some(e.line_count)
}

fn delimited_line_count_uncached(
    right_content: &RightPaneContent,
    content_width: u16,
) -> Option<usize> {
    if !viewer_show_delimited_table(right_content) {
        return None;
    }
    let raw = right_content.viewer.as_deref()?;
    let vp = right_content.snap_meta.path.as_deref()?;
    let rows = viewers::csv_handler::parse_csv(raw, Some(vp)).ok()?;
    if rows.is_empty() {
        return None;
    }
    if viewers::csv_handler::should_render_as_table(&rows) {
        return Some(viewers::csv_handler::table_line_count(&rows, content_width));
    }
    let total_rows_hint = viewers::csv_handler::total_rows_hint_from_raw(raw);
    Some(
        viewers::csv_handler::wide_structured_string(&rows, content_width, total_rows_hint)
            .lines()
            .count(),
    )
}

fn raster_viewer_line_count(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
) -> usize {
    if state.viewer_image.protocol.is_some() {
        return 1;
    }
    if state.viewer_image.decode_rx.is_some() && state.viewer_image.err.is_none() {
        return wrapped_line_count(
            &viewers::images::raster_preview_label_body(right_content, UI_STRINGS.misc.loading),
            content_width,
        ) as usize;
    }
    if let Some(e) = state.viewer_image.err.as_deref() {
        return wrapped_line_count(
            &viewers::images::raster_preview_label_body(right_content, e),
            content_width,
        ) as usize;
    }
    let msg = right_content.viewer.as_deref().unwrap_or("");
    wrapped_line_count(
        &viewers::images::raster_preview_label_body(right_content, msg),
        content_width,
    ) as usize
}

fn tree_with_policy_line_count(right_content: &RightPaneContent, content_width: u16) -> usize {
    let tree_lines = right_content
        .viewer
        .as_deref()
        .map_or(0, |t| wrapped_line_count(t, content_width) as usize);
    if let Some(ref pl) = right_content.viewer_directory_policy_line {
        let policy_lines = wrapped_line_count(pl, content_width) as usize;
        return policy_lines.saturating_add(1).saturating_add(tree_lines);
    }
    tree_lines
}

fn viewer_uses_preformatted_layout(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
) -> bool {
    if state.right_pane_mode != RightPaneMode::Viewer {
        return false;
    }
    if right_content.snap_meta.path.is_none() {
        return false;
    }
    if viewer_is_markdown(right_content) {
        return true;
    }
    if viewer_uses_syntect_highlight(right_content) {
        return true;
    }
    if viewers::images::is_raster_preview_category(right_content)
        && state.viewer_image.protocol.is_some()
    {
        return true;
    }
    if !viewer_show_delimited_table(right_content) {
        return false;
    }
    let theme = themes::current().name;
    if csv_cached_entry(state, right_content, content_width, theme).is_some() {
        return true;
    }
    let Some(raw) = right_content.viewer.as_deref() else {
        return false;
    };
    viewers::csv_handler::parse_csv(raw, right_content.snap_meta.path.as_deref())
        .is_ok_and(|r| !r.is_empty())
}

pub fn ratatui_wrap_right_paragraph(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
) -> bool {
    !viewer_uses_preformatted_layout(state, right_content, content_width)
}

fn wrapped_line_count(text: &str, width: u16) -> u16 {
    let w = width as usize;
    if w == 0 {
        return 0;
    }
    text.lines()
        .map(|line| (line.chars().count().div_ceil(w)).max(1))
        .sum::<usize>()
        .min(u16::MAX as usize) as u16
}

#[inline]
fn text_from_viewer_raw(raw: &str) -> Text<'static> {
    Text::from(raw.to_string())
}

/// Bold + italic policy line, blank line, then tree body (plain lines).
fn text_directory_policy_and_tree(policy_line: &str, tree_body: &str) -> Text<'static> {
    let line_style = style::text_style().add_modifier(Modifier::BOLD | Modifier::ITALIC);
    let mut lines = vec![
        Line::from(vec![Span::styled(policy_line.to_string(), line_style)]),
        Line::default(),
    ];
    lines.extend(text_from_viewer_raw(tree_body).lines);
    Text::from(lines)
}

#[inline]
fn text_from_cache_entry(
    entry: &ViewerTextCacheEntry,
    text_viewport: Option<(u16, u16)>,
) -> Text<'static> {
    match text_viewport {
        Some((sy, vh)) => entry.viewport_text(sy, vh),
        None => entry.text.clone(),
    }
}

fn viewer_display_text(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
    text_viewport: Option<(u16, u16)>,
) -> Text<'static> {
    let raw = right_content
        .viewer
        .as_deref()
        .unwrap_or(UI_STRINGS.pane.viewer_placeholder);
    if right_content.snap_meta.path.is_some() {
        if viewer_show_delimited_table(right_content) {
            let theme = themes::current().name;
            if raw.len() >= cache::VIEWER_TEXT_CACHE.min_csv_bytes
                && viewers::async_tools::viewer_async_placeholder_active(state, right_content)
            {
                return text_from_viewer_raw(raw);
                // return Text::from(UI_STRINGS.loading.general.to_string());
            }
            if let Some(e) = csv_cached_entry(state, right_content, content_width, theme) {
                return text_from_cache_entry(e, text_viewport);
            }
            if let Ok(rows) =
                viewers::csv_handler::parse_csv(raw, right_content.snap_meta.path.as_deref())
                && !rows.is_empty()
            {
                if viewers::csv_handler::should_render_as_table(&rows) {
                    return viewers::csv_handler::table_to_text(&rows, content_width);
                }
                let total_rows_hint = viewers::csv_handler::total_rows_hint_from_raw(raw);
                return viewers::csv_handler::wide_structured_text(
                    &rows,
                    content_width,
                    total_rows_hint,
                );
            }
        } else if viewer_is_markdown(right_content) {
            let theme = themes::current().name;
            if raw.len() >= cache::VIEWER_TEXT_CACHE.min_markdown_bytes
                && viewers::async_tools::viewer_async_placeholder_active(state, right_content)
            {
                return text_from_viewer_raw(raw);
            }
            if raw.len() >= cache::VIEWER_TEXT_CACHE.min_markdown_bytes
                && let Some(ref path) = right_content.snap_meta.path
                && let Some(ref e) = state.viewer_text_cache
                && e.matches_markdown_viewer(path.as_str(), content_width, theme, raw)
            {
                return text_from_cache_entry(e, text_viewport);
            }
            let doc = viewers::markdown::parse_markdown(raw);
            return doc.to_text(content_width);
        } else if viewer_uses_syntect_highlight(right_content) {
            let Some(path) = right_content.snap_meta.path.as_deref() else {
                return text_from_viewer_raw(raw);
            };
            let theme = themes::current().name;
            let syntect_args =
                syntect_viewer_cache_params(path, raw, content_width, theme, right_content);
            let cat = syntect_args.category;
            if raw.len() >= cache::VIEWER_TEXT_CACHE.min_syntect_bytes
                && viewers::async_tools::viewer_async_placeholder_active(state, right_content)
            {
                return text_from_viewer_raw(raw);
            }
            if raw.len() >= cache::VIEWER_TEXT_CACHE.min_syntect_bytes
                && let Some(ref e) = state.viewer_text_cache
                && e.matches_syntect_viewer(&syntect_args)
            {
                return text_from_cache_entry(e, text_viewport);
            }
            return viewers::syntect_text::highlight_viewer(raw, path, cat);
        } else if viewers::images::is_raster_preview_category(right_content) {
            if state.viewer_image.protocol.is_some() {
                return Text::default();
            }
            if state.viewer_image.decode_rx.is_some() && state.viewer_image.err.is_none() {
                return Text::from(viewers::images::raster_preview_label_body(
                    right_content,
                    UI_STRINGS.misc.loading,
                ));
            }
            if let Some(e) = state.viewer_image.err.as_deref() {
                return Text::from(viewers::images::raster_preview_label_body(right_content, e));
            }
            let msg = right_content.viewer.as_deref().unwrap_or("");
            return Text::from(viewers::images::raster_preview_label_body(
                right_content,
                msg,
            ));
        }
    }
    if let Some(ref policy_line) = right_content.viewer_directory_policy_line {
        return text_directory_policy_and_tree(policy_line, raw);
    }
    text_from_viewer_raw(raw)
}

pub fn content_display_text(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
    text_viewport: Option<(u16, u16)>,
) -> Text<'static> {
    match state.right_pane_mode {
        RightPaneMode::Viewer => {
            viewer_display_text(state, right_content, content_width, text_viewport)
        }
        RightPaneMode::Templates => Text::from(right_content.templates.clone()),
        RightPaneMode::Metadata => Text::from(
            right_content
                .metadata
                .clone()
                .unwrap_or_else(|| UI_STRINGS.pane.not_available.to_string()),
        ),
        RightPaneMode::Writing => Text::from(
            right_content
                .writing
                .clone()
                .unwrap_or_else(|| UI_STRINGS.pane.not_available.to_string()),
        ),
    }
}

#[must_use]
pub fn text_to_plain_string(text: &Text<'_>) -> String {
    let mut s = String::new();
    for (i, line) in text.lines.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        for span in &line.spans {
            s.push_str(span.content.as_ref());
        }
    }
    s
}

/// Haystack for Metadata / Writing tabs: formatted table text (same as on-screen cells), not raw JSON.
/// `content_width` should match the tab’s text width so array formatting matches [`kv_tables::draw::draw_tables`].
#[must_use]
pub fn json_tab_find_haystack(
    right_content: &RightPaneContent,
    mode: RightPaneMode,
    content_width: u16,
    column_stats: crate::config::ColumnStatsDisplay,
) -> Option<String> {
    let json = match mode {
        RightPaneMode::Metadata => right_content.metadata.as_deref(),
        RightPaneMode::Writing => right_content.writing.as_deref(),
        _ => None,
    }?;
    if json.trim().is_empty() {
        return None;
    }
    let ctx = kv_tables::KvParseCtx::from_content_width(content_width, column_stats);
    Some(kv_tables::searchable_text_from_json_with_ctx(json, ctx))
}

#[must_use]
pub fn viewer_find_haystack_text(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
) -> String {
    ensure_viewer_text_cache(state, right_content, content_width);
    if let Some(h) = json_tab_find_haystack(
        right_content,
        state.right_pane_mode,
        content_width,
        state.panels.column_stats,
    ) {
        return h;
    }
    text_to_plain_string(&content_display_text(
        state,
        right_content,
        content_width,
        None,
    ))
}

/// Number of non-overlapping literal occurrences of `needle` in `haystack`.
#[must_use]
pub fn literal_match_count(haystack: &str, needle: &str) -> usize {
    let needle = needle.trim();
    if needle.is_empty() {
        return 0;
    }
    haystack.match_indices(needle).count()
}

/// Plain text for a given right-pane tab (restores active tab and cache after).
#[must_use]
pub fn haystack_for_right_pane_mode(
    state: &mut UblxState,
    right_content: &RightPaneContent,
    content_width: u16,
    mode: RightPaneMode,
) -> String {
    let saved = state.right_pane_mode;
    state.right_pane_mode = mode;
    ensure_viewer_text_cache(state, right_content, content_width);
    let out = if let Some(h) = json_tab_find_haystack(
        right_content,
        mode,
        content_width,
        state.panels.column_stats,
    ) {
        h
    } else {
        text_to_plain_string(&content_display_text(
            state,
            right_content,
            content_width,
            None,
        ))
    };
    state.right_pane_mode = saved;
    ensure_viewer_text_cache(state, right_content, content_width);
    out
}
