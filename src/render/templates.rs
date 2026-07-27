//! Structured zahirscan Templates tab (THI-177).
//!
//! Shared [`TemplateView`] parse used by TUI (and later serve / web). Not routed through
//! [`crate::render::kv_tables`] — templates have a fixed schema.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Write as _;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::layout::style;
use crate::themes;

/// One zahirscan template row for UI (pattern / count / sorted examples).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateView {
    pub pattern: String,
    pub count: usize,
    /// Placeholder → example values; keys sorted (matches zahirscan `BTreeMap` JSON order).
    pub examples: Vec<(String, Vec<String>)>,
}

#[derive(Deserialize)]
struct RawTemplate {
    pattern: String,
    count: usize,
    #[serde(default)]
    examples: std::collections::BTreeMap<String, Vec<String>>,
}

impl From<RawTemplate> for TemplateView {
    fn from(raw: RawTemplate) -> Self {
        Self {
            pattern: raw.pattern,
            count: raw.count,
            examples: raw.examples.into_iter().collect(),
        }
    }
}

/// Parse `templates` array from a zahir JSON value. Empty on missing/invalid.
#[must_use]
pub fn template_views_from_value(value: &Value) -> Vec<TemplateView> {
    let Some(arr) = value.get("templates") else {
        return Vec::new();
    };
    template_views_from_json_value(arr)
}

/// Parse a templates JSON array (or the pretty-printed Templates tab string).
///
/// Drops empty/whitespace-only patterns (zahirscan blank-line noise until fixed upstream).
#[must_use]
pub fn template_views_from_json_value(value: &Value) -> Vec<TemplateView> {
    match serde_json::from_value::<Vec<RawTemplate>>(value.clone()) {
        Ok(rows) => rows
            .into_iter()
            .filter(|r| !r.pattern.trim().is_empty())
            .map(TemplateView::from)
            .collect(),
        _ => Vec::new(),
    }
}

/// Parse the Templates tab body (pretty-printed JSON array). Empty → treat as unstructured.
#[must_use]
pub fn template_views_from_templates_str(templates: &str) -> Vec<TemplateView> {
    let trimmed = templates.trim();
    if trimmed.is_empty() || trimmed == "null" || trimmed == "[]" {
        return Vec::new();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => template_views_from_json_value(&v),
        Err(_) => Vec::new(),
    }
}

/// Find / copy haystack from structured views (patterns + example values).
#[must_use]
pub fn templates_searchable_text(views: &[TemplateView]) -> String {
    let mut out = String::new();
    for (i, t) in views.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&t.pattern);
        let _ = write!(out, " ×{}", t.count);
        for (ph, vals) in &t.examples {
            out.push('\n');
            out.push_str(ph);
            out.push('\t');
            out.push_str(&vals.join(", "));
        }
    }
    out
}

/// Split pattern into spans; `[PLACEHOLDER]` tokens use muted/accent chrome.
#[must_use]
pub fn pattern_spans(pattern: &str, base: Style, placeholder: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let bytes = pattern.as_bytes();
    let mut i = 0;
    let mut plain_start = 0;
    while i < bytes.len() {
        if bytes[i] == b'['
            && let Some(rel) = pattern[i..].find(']')
        {
            let end = i + rel + 1;
            if plain_start < i {
                spans.push(Span::styled(pattern[plain_start..i].to_string(), base));
            }
            spans.push(Span::styled(pattern[i..end].to_string(), placeholder));
            i = end;
            plain_start = end;
            continue;
        }
        i += 1;
    }
    if plain_start < pattern.len() {
        spans.push(Span::styled(pattern[plain_start..].to_string(), base));
    }
    if spans.is_empty() {
        spans.push(Span::styled(pattern.to_string(), base));
    }
    spans
}

/// Word-wrap styled spans into lines that fit `max_width` terminal columns.
#[must_use]
pub fn wrap_styled_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Line<'static>> {
    let max_width = max_width.max(1);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut cur: Vec<Span<'static>> = Vec::new();
    let mut cur_w = 0usize;

    let flush =
        |cur: &mut Vec<Span<'static>>, cur_w: &mut usize, lines: &mut Vec<Line<'static>>| {
            if cur.is_empty() {
                return;
            }
            lines.push(Line::from(std::mem::take(cur)));
            *cur_w = 0;
        };

    for span in spans {
        let style = span.style;
        let text = span.content.into_owned();
        // Prefer breaking on whitespace; otherwise hard-break grapheme clusters by width.
        for word in text.split_inclusive(char::is_whitespace) {
            let mut rest = word.to_string();
            while !rest.is_empty() {
                let w = rest.width();
                if cur_w + w <= max_width {
                    cur.push(Span::styled(rest, style));
                    cur_w += w;
                    break;
                }
                if cur_w == 0 {
                    // Hard-break oversized token.
                    let mut take = 0usize;
                    let mut cols = 0usize;
                    for (i, ch) in rest.char_indices() {
                        let cw = ch.width().unwrap_or(0);
                        if cols + cw > max_width && take > 0 {
                            break;
                        }
                        cols += cw;
                        take = i + ch.len_utf8();
                    }
                    let (head, tail) = rest.split_at(take.max(1.min(rest.len())));
                    cur.push(Span::styled(head.to_string(), style));
                    flush(&mut cur, &mut cur_w, &mut lines);
                    rest = tail.to_string();
                } else {
                    flush(&mut cur, &mut cur_w, &mut lines);
                }
            }
        }
    }
    flush(&mut cur, &mut cur_w, &mut lines);
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

fn pattern_lines(
    pattern: &str,
    count: usize,
    max_width: usize,
    base: Style,
    placeholder: Style,
    count_style: Style,
) -> Vec<Line<'static>> {
    // Reserve for highlight symbol ("▶ ") when measuring wrap width.
    let wrap_w = max_width.saturating_sub(2).max(8);
    let mut spans = pattern_spans(pattern, base, placeholder);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(format!("×{count}"), count_style));
    let mut lines = wrap_styled_spans(spans, wrap_w);
    // Slight vpad after wrapped patterns only — one-liners stay dense.
    if lines.len() > 1 {
        lines.push(Line::from(""));
    }
    lines
}

fn hrule_line(width: usize, style: Style, mark: Option<char>) -> Line<'static> {
    let w = width.max(1);
    let Some(ch) = mark else {
        return Line::from(Span::styled("─".repeat(w), style));
    };
    // "↓ ─…─ ↓" — same marker on both ends when that direction can scroll.
    let mark_cols = 4usize; // "X " + " X"
    let rule_cols = w.saturating_sub(mark_cols).max(1);
    Line::from(vec![
        Span::styled(format!("{ch} "), style),
        Span::styled("─".repeat(rule_cols), style),
        Span::styled(format!(" {ch}"), style),
    ])
}

fn truncate_placeholder(ph: &str, cols: usize) -> String {
    if ph.width() <= cols {
        return format!("{ph:<cols$}");
    }
    let mut take = 0usize;
    let mut used = 0usize;
    for (i, ch) in ph.char_indices() {
        let cw = ch.width().unwrap_or(0);
        if used + cw > cols.saturating_sub(1) && take > 0 {
            break;
        }
        used += cw;
        take = i + ch.len_utf8();
    }
    format!("{:<cols$}", format!("{}…", &ph[..take.max(1)]))
}

/// Two-column example rows (header + placeholder/values); values wrap.
fn example_body_lines(
    examples: &[(String, Vec<String>)],
    content_width: usize,
    base: Style,
    placeholder: Style,
    hint: Style,
) -> Vec<Line<'static>> {
    const PH_COLS: usize = 16;
    if examples.is_empty() {
        return vec![Line::from(Span::styled("(no examples)", hint))];
    }
    let gap = 2usize;
    let wrap_w = content_width.saturating_sub(2).max(8);
    let val_cols = wrap_w.saturating_sub(PH_COLS + gap).max(8);
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(format!("{:<PH_COLS$}", "placeholder"), hint),
        Span::raw(" ".repeat(gap)),
        Span::styled("values", hint),
    ]));
    for (ph, vals) in examples {
        let ph_label = truncate_placeholder(ph, PH_COLS);
        let val_text = vals.join(", ");
        let wrapped = wrap_styled_spans(vec![Span::styled(val_text, base)], val_cols);
        for (i, line) in wrapped.into_iter().enumerate() {
            let mut spans = Vec::with_capacity(2 + line.spans.len());
            if i == 0 {
                spans.push(Span::styled(ph_label.clone(), placeholder));
            } else {
                spans.push(Span::raw(" ".repeat(PH_COLS)));
            }
            spans.push(Span::raw(" ".repeat(gap)));
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        }
    }
    lines
}

/// Max visible example-body lines (between rules) so the selected pattern stays on screen.
fn examples_body_cap(area_height: u16) -> usize {
    let h = area_height as usize;
    // Leave room for at least one pattern line + two rules; cap body at ~2/3 of pane.
    let leave = 4usize; // pattern + 2 rules + breathing room
    let by_fraction = (h * 2 / 3).max(3);
    by_fraction.min(h.saturating_sub(leave)).max(1)
}

fn apply_line_style(line: Line<'static>, style: Style) -> Line<'static> {
    Line::from(
        line.spans
            .into_iter()
            .map(|s| Span::styled(s.content, s.style.patch(style)))
            .collect::<Vec<_>>(),
    )
}

fn with_gutter(line: Line<'static>, selected_first: bool, highlight: Style) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len() + 1);
    if selected_first {
        spans.push(Span::styled("▶ ", highlight));
    } else {
        spans.push(Span::raw("  "));
    }
    spans.extend(line.spans);
    Line::from(spans)
}

/// Draw one Patterns list; selected row expands examples inline between two horizontal rules.
///
/// Only the selected **pattern** lines use list highlight; the framed examples stay unhighlighted.
pub fn draw_templates(
    f: &mut Frame,
    area: Rect,
    views: &[TemplateView],
    list_state: &mut ListState,
    examples_scroll: u16,
) {
    if views.is_empty() || area.height < 3 {
        return;
    }
    let n = views.len();
    let sel = list_state.selected().unwrap_or(0).min(n.saturating_sub(1));
    list_state.select(Some(sel));

    let pal = themes::current();
    let base = Style::default().fg(pal.text);
    let placeholder = Style::default()
        .fg(pal.title_brand)
        .add_modifier(Modifier::DIM);
    let count_style = Style::default().fg(pal.hint);
    let hint = Style::default().fg(pal.hint);
    let highlight = style::list_highlight();
    let wrap_cols = area.width as usize;
    let body_cap = examples_body_cap(area.height);

    let items: Vec<ListItem> = views
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let selected = i == sel;
            let mut lines: Vec<Line<'static>> = pattern_lines(
                &t.pattern,
                t.count,
                wrap_cols,
                base,
                placeholder,
                count_style,
            )
            .into_iter()
            .enumerate()
            .map(|(li, line)| {
                let styled = if selected {
                    apply_line_style(line, highlight)
                } else {
                    line
                };
                with_gutter(styled, selected && li == 0, highlight)
            })
            .collect();

            if selected {
                let rule_w = wrap_cols.saturating_sub(2).max(1);
                let body = example_body_lines(&t.examples, wrap_cols, base, placeholder, hint);
                let max_scroll = body.len().saturating_sub(body_cap);
                let scroll = (examples_scroll as usize).min(max_scroll);
                let end = (scroll + body_cap).min(body.len());
                let more_above = scroll > 0;
                let more_below = end < body.len();
                lines.push(with_gutter(
                    hrule_line(rule_w, hint, more_above.then_some('↑')),
                    false,
                    highlight,
                ));
                lines.extend(
                    body[scroll..end]
                        .iter()
                        .cloned()
                        .map(|line| with_gutter(line, false, highlight)),
                );
                lines.push(with_gutter(
                    hrule_line(rule_w, hint, more_below.then_some('↓')),
                    false,
                    highlight,
                ));
            }
            ListItem::new(lines)
        })
        .collect();

    // Selection chrome is painted on pattern lines only; keep List highlight inert.
    let list = List::new(items)
        .highlight_style(Style::default())
        .highlight_symbol("");
    f.render_stateful_widget(list, area, list_state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_zahir_templates_array() {
        let v = json!({
            "templates": [
                {
                    "pattern": "[DATE] ERROR: [MSG]",
                    "count": 3,
                    "examples": {
                        "DATE": ["2026-01-01"],
                        "MSG": ["boom", "fail"]
                    }
                }
            ]
        });
        let views = template_views_from_value(&v);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].pattern, "[DATE] ERROR: [MSG]");
        assert_eq!(views[0].count, 3);
        assert_eq!(views[0].examples[0].0, "DATE");
        assert_eq!(views[0].examples[1].1, vec!["boom", "fail"]);
    }

    #[test]
    fn drops_empty_pattern_rows() {
        let v = json!([
            { "pattern": "", "count": 5, "examples": {} },
            { "pattern": "   ", "count": 2, "examples": {} },
            {
                "pattern": "[POS_00]",
                "count": 1,
                "examples": { "POS_00": ["ok"] }
            }
        ]);
        let views = template_views_from_json_value(&v);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].pattern, "[POS_00]");
    }

    #[test]
    fn pattern_spans_splits_placeholders() {
        let spans = pattern_spans(
            "a [PID] b",
            Style::default(),
            Style::default().add_modifier(Modifier::BOLD),
        );
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content.as_ref(), "a ");
        assert_eq!(spans[1].content.as_ref(), "[PID]");
        assert_eq!(spans[2].content.as_ref(), " b");
    }

    #[test]
    fn wrap_styled_spans_breaks_long_line() {
        let spans = vec![Span::raw(
            "Connected to a-very-long-hostname.example.com:8080 with more text",
        )];
        let lines = wrap_styled_spans(spans, 20);
        assert!(lines.len() > 1);
        for line in &lines {
            let w: usize = line.spans.iter().map(|s| s.content.width()).sum();
            assert!(w <= 20, "line wider than wrap: {w}");
        }
    }
}
