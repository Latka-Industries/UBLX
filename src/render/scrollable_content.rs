//! Reusable layout and scrollbar for scrollable content (viewer, metadata, writing, templates, or any panel).
//! Callers provide total content size (e.g. line count); we reserve scrollbar when needed, apply padding, and clamp scroll.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::ScrollbarState;

use crate::layout::style;

/// Region inset by the same pad on top and bottom (Templates structured view).
#[must_use]
pub fn area_with_vertical_pad(area: Rect, pad: u16) -> Rect {
    let pad2 = pad.saturating_mul(2);
    if area.height > pad2 {
        let chunks = style::split_vertical(
            area,
            &[
                Constraint::Length(pad),
                Constraint::Min(0),
                Constraint::Length(pad),
            ],
        );
        chunks[1]
    } else {
        area
    }
}

/// Region above the bottom padding strip (same geometry as the first split inside [`layout_scrollable_content`]).
#[must_use]
pub fn area_above_bottom_pad(area: Rect, bottom_pad: u16) -> Rect {
    if area.height > bottom_pad {
        let chunks =
            style::split_vertical(area, &[Constraint::Min(0), Constraint::Length(bottom_pad)]);
        chunks[0]
    } else {
        area
    }
}

/// Text viewport width: full width of `padded`, or one column less when a vertical scrollbar is shown.
#[must_use]
pub fn viewport_text_width(padded: Rect, total_lines: usize) -> u16 {
    let viewport = padded.height as usize;
    let need_scrollbar = total_lines > viewport;
    if need_scrollbar && padded.width > 1 {
        padded.width - 1
    } else {
        padded.width
    }
}

/// Result of laying out a scrollable content area: where to draw content, where to draw the scrollbar, and the clamped scroll offset.
#[derive(Debug)]
pub struct ScrollableLayout {
    /// Rect for the main content (already inset by bottom pad and by scrollbar column when present). Draw content here; respect this as the viewport.
    pub content_rect: Rect,
    /// Rect for the scrollbar thumb (zero width/height when scrollbar not shown).
    pub scrollbar_rect: Rect,
    /// Clamped scroll offset (0..= `max_scroll`). Caller should write this back to state.
    pub scroll_y: u16,
    /// Whether a scrollbar is being shown (content exceeds viewport).
    pub show_scrollbar: bool,
}

/// Lays out a scrollable region: applies bottom padding, reserves scrollbar column when content is taller than viewport, and clamps `scroll` in place.
/// Use for any panel (right pane viewer/metadata/writing/templates, or others) that shows content which may be longer than the area.
///
/// * `area` – full content area (e.g. the pane body after tabs).
/// * `total_lines` – total number of logical lines (or rows) of content (for viewer: wrapped line count; for `kv_tables`: `content_height`; etc.).
/// * `scroll` – current scroll position; will be clamped to valid range and updated in place.
/// * `bottom_pad` – number of lines to reserve at the bottom (e.g. `style::UI_CONSTANTS.v_pad`).
pub fn layout_scrollable_content(
    area: Rect,
    total_lines: usize,
    scroll: &mut u16,
    bottom_pad: u16,
) -> ScrollableLayout {
    let content_with_pad = if area.height > bottom_pad {
        let chunks =
            style::split_vertical(area, &[Constraint::Min(0), Constraint::Length(bottom_pad)]);
        chunks[0]
    } else {
        area
    };

    let viewport = content_with_pad.height as usize;
    let need_scrollbar = total_lines > viewport;

    let (content_rect, scrollbar_rect) = if need_scrollbar && content_with_pad.width > 1 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(content_with_pad);
        (chunks[0], chunks[1])
    } else {
        (content_with_pad, Rect::default())
    };

    let viewport_final = content_rect.height as usize;
    let max_scroll = total_lines.saturating_sub(viewport_final) as u16;
    let scroll_y = (*scroll).min(max_scroll);
    *scroll = scroll_y;

    ScrollableLayout {
        content_rect,
        scrollbar_rect,
        scroll_y,
        show_scrollbar: need_scrollbar,
    }
}

/// Renders the vertical scrollbar in `layout.scrollbar_rect` when `layout.show_scrollbar` and rect has size.
/// Uses the same style as the viewer scrollbar.
pub fn draw_scrollbar(f: &mut ratatui::Frame, layout: &ScrollableLayout, total_lines: usize) {
    if !layout.show_scrollbar
        || layout.scrollbar_rect.width == 0
        || layout.scrollbar_rect.height == 0
    {
        return;
    }
    let viewport = layout.content_rect.height as usize;
    let max_scroll = total_lines.saturating_sub(viewport);
    if max_scroll == 0 {
        return;
    }
    let content_len = max_scroll + 1;
    let mut state = ScrollbarState::new(content_len)
        .position(layout.scroll_y as usize)
        .viewport_content_length(1);
    f.render_stateful_widget(style::viewer_scrollbar(), layout.scrollbar_rect, &mut state);
}
