//! Layout and paint for the right pane: scrollable body, tab row, and public
//! [`draw_right_pane`] / [`draw_right_pane_fullscreen`].
//!
//! Delegates text/raster decisions to [`super::content`] and frame/tabs/footers to [`super::chrome`].
//! Imports the sibling modules by path (`chrome::…`, `content::…`) instead of listing every symbol.

use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::layout::{
    setup::{RightPaneContent, RightPaneMode, UblxState, ViewData},
    style,
};
use crate::modules::viewer_search;
use crate::render::viewers::images;
use crate::render::{kv_tables, scrollable_content, templates};
use crate::ui::UI_CONSTANTS;

use super::chrome;
use super::content;

fn draw_right_pane_scrollable_body(
    f: &mut ratatui::Frame,
    state: &mut UblxState,
    right_content: &RightPaneContent,
    scroll_area: Rect,
    bottom_pad: u16,
) {
    let use_kv_tables = match state.right_pane_mode {
        RightPaneMode::Metadata => right_content.metadata.as_deref(),
        RightPaneMode::Writing => right_content.writing.as_deref(),
        _ => None,
    };
    let use_template_views = state.right_pane_mode == RightPaneMode::Templates
        && !right_content.template_views.is_empty();

    if use_template_views {
        // Structured Templates: 1 top + 1 bottom (symmetric); other tabs stay bottom-only.
        let body = scrollable_content::area_with_vertical_pad(scroll_area, bottom_pad.max(1));
        let n = right_content.template_views.len();
        if state.panels.template_list.selected().is_none_or(|i| i >= n) {
            state.panels.template_list.select(Some(0));
        }
        templates::draw_templates(
            f,
            body,
            &right_content.template_views,
            &mut state.panels.template_list,
            state.panels.preview_scroll,
        );
        state.panels.right_pane_text_w = Some(body.width);
        return;
    }

    let padded = scrollable_content::area_above_bottom_pad(scroll_area, bottom_pad);

    images::ensure_viewer_image(state, right_content, Some((padded.width, padded.height)));
    let mut text_w = padded.width;
    for _ in 0..6 {
        // Do not schedule or poll async viewer work here — `text_w` is still converging.
        // [`content::ensure_viewer_text_cache`] runs once after convergence and polls + schedules.
        let guess_lines = content::viewer_total_lines(
            right_content,
            text_w,
            use_kv_tables,
            state,
            Some(padded.height),
        );
        let w_next = scrollable_content::viewport_text_width(padded, guess_lines);
        if w_next == text_w {
            break;
        }
        text_w = w_next;
    }
    content::ensure_viewer_text_cache(state, right_content, text_w);
    let total_lines = content::viewer_total_lines(
        right_content,
        text_w,
        use_kv_tables,
        state,
        Some(padded.height),
    );
    viewer_search::sync(state, right_content, text_w, padded.height);
    let layout = scrollable_content::layout_scrollable_content(
        scroll_area,
        total_lines,
        &mut state.panels.preview_scroll,
        bottom_pad,
    );
    let text_rect = layout.content_rect;
    let find_active = state.viewer_find.find_affects_view();
    if let Some(json) = use_kv_tables {
        let needle = find_active.then_some(state.viewer_find.query.as_str());
        kv_tables::draw_tables(f, text_rect, json, layout.scroll_y, needle, state);
    } else {
        let image_mode = state.right_pane_mode == RightPaneMode::Viewer
            && images::is_raster_preview_category(right_content)
            && right_content.snap_meta.path.is_some()
            && state.viewer_image.protocol.is_some();
        if image_mode {
            if let Some(proto) = state.viewer_image.protocol.as_mut() {
                f.render_stateful_widget(images::stateful_widget(), text_rect, proto);
                let _ = proto.last_encoding_result();
            }
        } else {
            let text_vp = content::viewer_text_cache_viewport_active(state, right_content, text_w)
                .then_some((layout.scroll_y, text_rect.height));
            // When find is active, use full highlighted text + paragraph scroll (same as plain text).
            // CSV / markdown-cache viewport paths skip `text_vp` so substring highlights match the haystack.
            let use_highlighted_find = find_active;
            let para_scroll = if use_highlighted_find {
                (layout.scroll_y, 0)
            } else if text_vp.is_some() {
                (0, 0)
            } else {
                (layout.scroll_y, 0)
            };
            let body = if use_highlighted_find {
                let hay = content::viewer_find_haystack_text(state, right_content, text_w);
                viewer_search::highlighted_body(state, &hay)
            } else {
                content::content_display_text(state, right_content, text_w, text_vp)
            };
            let mut paragraph = Paragraph::new(body)
                .style(style::text_style())
                .scroll(para_scroll);
            if content::ratatui_wrap_right_paragraph(state, right_content, text_w) {
                paragraph = paragraph.wrap(Wrap { trim: false });
            }
            f.render_widget(paragraph, text_rect);
        }
    }
    scrollable_content::draw_scrollbar(f, &layout, total_lines);
    state.panels.right_pane_text_w = Some(text_w);
}

/// Draw the right (viewer) pane. `chunks` must have at least 3 elements; the right pane uses `chunks[2]`.
pub fn draw_right_pane(
    f: &mut ratatui::Frame,
    state: &mut UblxState,
    right_content: &RightPaneContent,
    chunks: &[Rect],
    transparent_page_chrome: bool,
) {
    let area = chunks[2];
    let inner_w = chrome::right_pane_inner_content_width(area);
    let text_w = state
        .panels
        .right_pane_text_w
        .unwrap_or_else(|| inner_w.saturating_sub(4));
    let footer_line =
        chrome::right_pane_bottom_line(state, right_content, inner_w, transparent_page_chrome);
    let right_block = chrome::right_pane_block(None, footer_line.as_ref());
    let tabs = chrome::visible_tabs(right_content);
    if !tabs.is_empty() && !tabs.iter().any(|(m, _)| *m == state.right_pane_mode) {
        state.right_pane_mode = tabs[0].0;
    }
    let tab_spans: Vec<Span> = chrome::right_pane_tab_spans(state, right_content, &tabs, text_w);
    let right_inner = right_block.inner(area);
    let constraints = &[
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ][..];
    let right_split = style::split_vertical(right_inner, constraints);
    let tab_row_chunks = style::tab_row_padded(right_split[0]);
    let content_chunks = style::tab_row_padded(right_split[2]);

    f.render_widget(&right_block, area);
    f.render_widget(Paragraph::new(Line::from(tab_spans)), tab_row_chunks[1]);

    let content_area = content_chunks[1];
    let bottom_pad = UI_CONSTANTS.v_pad;
    draw_right_pane_scrollable_body(f, state, right_content, content_area, bottom_pad);
}

/// Draw the current right-pane tab in full screen (hide categories and contents). Esc to exit.
pub fn draw_right_pane_fullscreen(
    f: &mut ratatui::Frame,
    state: &mut UblxState,
    right_content: &RightPaneContent,
    view: &ViewData,
    area: Rect,
    transparent_page_chrome: bool,
) {
    let inner_w = chrome::right_pane_inner_content_width(area);
    let footer_line = chrome::right_pane_footer_line_fullscreen(
        state,
        right_content,
        view,
        inner_w,
        transparent_page_chrome,
    );
    let fullscreen_title =
        chrome::right_pane_fullscreen_title_line(state, right_content.snap_meta.path.as_deref());
    let block = chrome::right_pane_block(Some(fullscreen_title), Some(&footer_line));
    let inner = block.inner(area);
    let bottom_pad = UI_CONSTANTS.v_pad;
    f.render_widget(&block, area);
    draw_right_pane_scrollable_body(f, state, right_content, inner, bottom_pad);
}
