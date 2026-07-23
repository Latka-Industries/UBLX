//! Theme selector overlay: list of theme names; highlight shows theme preview.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
use unicode_width::UnicodeWidthStr;

use crate::layout::style;
use crate::themes;
use crate::ui::{UI_CONSTANTS, UI_GLYPHS, UI_STRINGS};
use crate::utils::StringObjTraits;

/// Draw centered popup with theme list; **Dark** / **Light** rows are centered in the inner width; theme name rows are left-aligned (swatch + name). See `themes::theme_ordered_list` for order.
pub fn render_theme_selector(f: &mut Frame, selected_index: usize) {
    let area = f.area();
    let entries = themes::theme_selector_entries();
    let rect = popup_rect(area, entries);
    f.render_widget(Clear, rect);

    let inner_w = rect.width.saturating_sub(2) as usize;
    let current = themes::current();
    let block = theme_selector_block(current);
    let mut theme_row = 0usize;
    let items: Vec<ListItem<'_>> = entries
        .iter()
        .map(|entry| match entry {
            themes::SelectorEntry::Section(label) => section_row(label, current, inner_w),
            themes::SelectorEntry::Item(opt) => {
                let row = theme_option_row(theme_row, opt, selected_index, current);
                theme_row += 1;
                row
            }
        })
        .collect();

    f.render_widget(List::new(items).block(block), rect);
}

fn popup_rect(area: Rect, entries: &[themes::SelectorEntry]) -> Rect {
    let content_w = entries
        .iter()
        .map(|e| match e {
            themes::SelectorEntry::Section(label) => {
                UI_STRINGS.theme_selector_section_row(label).width()
            }
            themes::SelectorEntry::Item(t) => 2 + 2 + t.name.len(),
        })
        .max()
        .unwrap_or(0);
    let content_h = entries.len();
    style::centered_popup_rect(
        area,
        content_w,
        content_h,
        UI_CONSTANTS.popup_padding_w,
        UI_CONSTANTS.popup_padding_h,
    )
}

fn theme_selector_block(t: &themes::Palette) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(Line::from(UI_STRINGS.pad(UI_STRINGS.dialogs.theme)).centered())
        .border_style(Style::default().fg(t.focused_border))
        .style(Style::default().bg(t.popup_bg))
}

fn section_row(
    label: &'static str,
    current_theme: &themes::Palette,
    inner_width: usize,
) -> ListItem<'static> {
    let bg = current_theme.popup_bg;
    let style = Style::default().fg(current_theme.hint).bg(bg);
    let text = UI_STRINGS.theme_selector_section_row(label);
    let pad = inner_width
        .saturating_sub(text.width())
        .saturating_div(2)
        .saturating_sub(2);
    let padded = format!("{}{}", " ".repeat(pad), text);
    ListItem::new(Line::from(vec![Span::styled(padded, style)]))
}

fn theme_option_row(
    theme_row_index: usize,
    theme: &themes::Palette,
    selected_theme_index: usize,
    current_theme: &themes::Palette,
) -> ListItem<'static> {
    let swatch = themes::theme_selector_swatch(theme, current_theme);
    let swatch_style = Style::default().fg(swatch).bg(swatch);
    let (row_style, pad_style) =
        row_styles(theme_row_index, theme, selected_theme_index, current_theme);
    let line = Line::from(vec![
        UI_CONSTANTS.get_empty_span(pad_style),
        Span::styled(UI_GLYPHS.swatch_block.to_string(), swatch_style),
        UI_CONSTANTS.get_empty_span(pad_style),
        Span::styled(theme.name, row_style),
    ]);
    ListItem::new(line)
}

fn row_styles(
    theme_row_index: usize,
    theme: &themes::Palette,
    selected_theme_index: usize,
    current_theme: &themes::Palette,
) -> (Style, Style) {
    if theme_row_index == selected_theme_index {
        let bg = themes::node_pill_background(theme);
        (
            Style::default().fg(theme.focused_border).bg(bg),
            Style::default().fg(bg).bg(bg),
        )
    } else {
        let bg = current_theme.popup_bg;
        (
            Style::default().fg(current_theme.text).bg(bg),
            Style::default().fg(bg).bg(bg),
        )
    }
}
