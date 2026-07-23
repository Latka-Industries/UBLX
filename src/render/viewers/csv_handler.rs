//! Render delimiter-separated file content in the viewer as a table with box-drawing borders
//! (comfy-table). **Delimiter:** extension on the viewed path first (`.csv` → comma, `.tsv`/`.tab` →
//! tab, `.psv` → pipe) via [`crate::integrations::delimiter_from_path_for_viewer`]; if the
//! extension doesn’t decide, fall back to zahirscan’s [`crate::integrations::detect_delimiter_byte`]
//! on the file contents.
//!
//! Layout matches markdown tables: word wrap, short columns without wrap (still row-padded), and
//! [`crate::render::viewers::pretty_tables::VIEWER_TABLE_ELLIPSIS_CELL_CHARS`] truncation with `"..."`.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use std::fmt::Write as _;
use std::io::Cursor;

use crate::integrations::{delimiter_from_path_for_viewer, detect_delimiter_byte};
use crate::render::viewers::html_escape::html_escape_minimal;
use crate::render::viewers::pretty_tables;
use crate::themes;

/// Parsed delimited files wider than this render as raw text in the viewer.
pub const VIEWER_TABLE_MAX_COLUMNS: usize = 30;
const CSV_TOTAL_ROWS_META_PREFIX: &str = "__UBLX_CSV_TOTAL_ROWS__=";

/// True when parsed rows should render as a pretty table.
#[must_use]
pub fn should_render_as_table(rows: &[Vec<String>]) -> bool {
    !rows.is_empty() && rows.iter().map(Vec::len).max().unwrap_or(0) <= VIEWER_TABLE_MAX_COLUMNS
}

fn strip_total_rows_meta_row(rows: &mut Vec<Vec<String>>) {
    if rows
        .last()
        .is_some_and(|r| r.len() == 1 && r[0].trim().starts_with(CSV_TOTAL_ROWS_META_PREFIX))
    {
        rows.pop();
    }
}

/// Extract total-row hint from a synthetic trailer line emitted by viewer preview loading.
#[must_use]
pub fn total_rows_hint_from_raw(raw: &str) -> Option<usize> {
    let last = raw.lines().last()?.trim();
    let n = last.strip_prefix(CSV_TOTAL_ROWS_META_PREFIX)?;
    n.parse::<usize>().ok()
}

fn truncate_for_width(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= width {
        return s.to_string();
    }
    if width <= 3 {
        return s.chars().take(width).collect();
    }
    let keep = width - 3;
    format!("{}...", s.chars().take(keep).collect::<String>())
}

fn visible_cols_and_widths(rows: &[Vec<String>], content_width: u16) -> (usize, Vec<usize>, usize) {
    let total_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if total_cols == 0 {
        return (0, Vec::new(), 0);
    }

    let viewport = (content_width as usize).max(1);
    let sample_rows = rows.iter().take(256);
    let mut max_lens = vec![1usize; total_cols];
    for row in sample_rows {
        for (j, cell) in row.iter().enumerate() {
            let c = pretty_tables::collapse_viewer_cell_whitespace(cell);
            max_lens[j] = max_lens[j].max(c.chars().count());
        }
    }

    let mut widths = Vec::new();
    let mut used = 0usize;
    for max_len in max_lens {
        let w = max_len.clamp(3, 24);
        let sep = usize::from(!widths.is_empty()) * 3;
        if !widths.is_empty() && used + sep + w > viewport {
            break;
        }
        used += sep + w;
        widths.push(w);
    }
    if widths.is_empty() {
        widths.push(viewport.clamp(1, 24));
    }
    let visible = widths.len().min(total_cols);
    (visible, widths, total_cols)
}

fn row_to_structured_line(row: &[String], widths: &[usize]) -> String {
    widths
        .iter()
        .enumerate()
        .map(|(j, width)| {
            let raw = row.get(j).map_or("", String::as_str);
            let collapsed = pretty_tables::collapse_viewer_cell_whitespace(raw);
            let clipped = truncate_for_width(&collapsed, *width);
            format!("{clipped:<width$}")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn structured_cell_text(row: &[String], col_idx: usize, width: usize) -> String {
    let raw = row.get(col_idx).map_or("", String::as_str);
    let collapsed = pretty_tables::collapse_viewer_cell_whitespace(raw);
    let clipped = truncate_for_width(&collapsed, width);
    format!("{clipped:<width$}")
}

fn row_to_structured_spans(row: &[String], widths: &[usize]) -> Vec<Span<'static>> {
    let palette = themes::current();
    let base = Style::default().fg(palette.text);
    let alt = Style::default().fg(palette.hint);
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (j, width) in widths.iter().enumerate() {
        let padded = structured_cell_text(row, j, *width);
        let style = if j % 2 == 0 { base } else { alt };
        spans.push(Span::styled(padded, style));
        if j + 1 < widths.len() {
            spans.push(Span::styled(" | ".to_string(), base));
        }
    }
    spans
}

fn wide_structured_banner(
    shown_rows: usize,
    total_rows: usize,
    total_cols: usize,
    visible_cols: usize,
    hidden_rows: usize,
    hidden_cols: usize,
) -> String {
    format!(
        "[wide delimited view: truncated from {total_rows} rows and {total_cols} columns; rows {shown_rows} shown, {hidden_rows} hidden; columns {visible_cols} shown, {hidden_cols} hidden]"
    )
}

struct WideViewMetrics {
    visible_cols: usize,
    widths: Vec<usize>,
    total_cols: usize,
    shown_rows: usize,
    total_rows: usize,
    hidden_rows: usize,
    hidden_cols: usize,
}

fn wide_view_metrics(
    rows: &[Vec<String>],
    content_width: u16,
    total_rows_hint: Option<usize>,
) -> Option<WideViewMetrics> {
    if rows.is_empty() {
        return None;
    }
    let (visible_cols, widths, total_cols) = visible_cols_and_widths(rows, content_width);
    if visible_cols == 0 {
        return None;
    }
    let shown_rows = rows.len();
    let total_rows = total_rows_hint.unwrap_or(shown_rows).max(shown_rows);
    Some(WideViewMetrics {
        visible_cols,
        hidden_cols: total_cols.saturating_sub(visible_cols),
        widths,
        total_cols,
        shown_rows,
        total_rows,
        hidden_rows: total_rows.saturating_sub(shown_rows),
    })
}

/// For wide delimited files (more than [`VIEWER_TABLE_MAX_COLUMNS`] columns), render a structured
/// plain-text viewport: first visible columns only, aligned by column width, no horizontal paging.
#[must_use]
pub fn wide_structured_string(
    rows: &[Vec<String>],
    content_width: u16,
    total_rows_hint: Option<usize>,
) -> String {
    let Some(m) = wide_view_metrics(rows, content_width, total_rows_hint) else {
        return String::new();
    };

    let mut out = String::new();
    let _ = write!(
        out,
        "{}",
        wide_structured_banner(
            m.shown_rows,
            m.total_rows,
            m.total_cols,
            m.visible_cols,
            m.hidden_rows,
            m.hidden_cols
        )
    );
    out.push('\n');

    for row in rows {
        out.push_str(&row_to_structured_line(row, &m.widths));
        out.push('\n');
    }

    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Styled text variant of [`wide_structured_string`].
#[must_use]
pub fn wide_structured_text(
    rows: &[Vec<String>],
    content_width: u16,
    total_rows_hint: Option<usize>,
) -> Text<'static> {
    let Some(m) = wide_view_metrics(rows, content_width, total_rows_hint) else {
        return Text::default();
    };
    let palette = themes::current();
    let banner = wide_structured_banner(
        m.shown_rows,
        m.total_rows,
        m.total_cols,
        m.visible_cols,
        m.hidden_rows,
        m.hidden_cols,
    );

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(rows.len().saturating_add(1));
    lines.push(Line::from(vec![Span::styled(
        banner,
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::ITALIC),
    )]));
    lines.extend(
        rows.iter()
            .map(|row| Line::from(row_to_structured_spans(row, &m.widths))),
    );
    Text::from(lines)
}

/// Line count for [`wide_structured_string`].
#[must_use]
pub fn wide_structured_line_count(rows: &[Vec<String>], content_width: u16) -> usize {
    wide_structured_string(rows, content_width, None)
        .lines()
        .count()
}

/// Parse with a single-byte delimiter. Uses the **`csv`** crate via `::csv::` (avoids confusion with
/// this module’s name, `csv_handler`).
fn parse_with_delimiter(raw: &str, delim: u8) -> Result<Vec<Vec<String>>, ::csv::Error> {
    let mut rows = Vec::new();
    let mut rdr = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delim)
        .flexible(true)
        .from_reader(Cursor::new(raw));
    for result in rdr.records() {
        let record = result?;
        rows.push(record.iter().map(String::from).collect());
    }
    Ok(rows)
}

/// Parse raw delimiter-separated text into a grid (first row = header for [`table_string`]).
///
/// `path_hint` should be the viewed file path when known so extensions select the delimiter; if
/// [`None`] or the extension is unknown, zahirscan’s content sniffing is used.
///
/// # Errors
///
/// Returns [`csv::Error`] when a row cannot be read or parsed.
pub fn parse_csv(raw: &str, path_hint: Option<&str>) -> Result<Vec<Vec<String>>, ::csv::Error> {
    let hint = path_hint.unwrap_or("");
    let delim = delimiter_from_path_for_viewer(hint).unwrap_or_else(|| detect_delimiter_byte(raw));
    let mut rows = parse_with_delimiter(raw, delim)?;
    strip_total_rows_meta_row(&mut rows);
    Ok(rows)
}

/// Build a comfy-table string from parsed rows: UTF8 box-drawing style, cells truncated.
/// First row is the header; `content_width` constrains table width; cells capped at `max_cell_chars`.
#[must_use]
pub fn table_string_with_max_cell(
    rows: &[Vec<String>],
    content_width: u16,
    max_cell_chars: usize,
) -> String {
    pretty_tables::table_string_with_max_cell(rows, content_width, max_cell_chars)
}

/// Like [`table_string_with_max_cell`] but treats every row as a body row (no `set_header`).
/// Use for markdown tables so header and data each get their own line.
#[must_use]
pub fn table_string_rows_only(
    rows: &[Vec<String>],
    content_width: u16,
    max_cell_chars: usize,
) -> String {
    pretty_tables::table_string_rows_only(rows, content_width, max_cell_chars)
}

/// Build a comfy-table string from parsed rows (first row = header): smart wrap / ellipsis.
#[must_use]
pub fn table_string(rows: &[Vec<String>], content_width: u16) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut rows = rows.to_vec();
    let header = rows.remove(0);
    pretty_tables::table_string_header_body_smart_wrap(&header, &rows, content_width)
}

/// Table as styled [`Text`] for the viewer, using [`crate::themes::current`] text color for the whole table.
#[must_use]
pub fn table_to_text(rows: &[Vec<String>], content_width: u16) -> Text<'static> {
    table_string_to_text(&table_string(rows, content_width))
}

/// Turn a pre-rendered table string into styled [Text] (for cache path).
#[must_use]
pub fn table_string_to_text(table_str: &str) -> Text<'static> {
    pretty_tables::table_string_to_text(table_str)
}

/// Number of lines the table string will occupy (for scroll height).
#[must_use]
pub fn table_line_count(rows: &[Vec<String>], content_width: u16) -> usize {
    table_string(rows, content_width).lines().count()
}

/// Build table string and line count in one pass (for caching).
#[must_use]
pub fn table_string_and_line_count(rows: &[Vec<String>], content_width: u16) -> (String, usize) {
    let s = table_string(rows, content_width);
    let count = s.lines().count();
    (s, count)
}

/// Host HTML for web Viewer — same parse/delimiter rules as the TUI table path.
///
/// Unlike the TUI, **all columns** are emitted; the web Viewer uses frozen
/// H/V scrollbars (`.csv-viewer__hbar` / `__vbar`) plus a pinned header row
/// (`.csv-viewer__head`) with `translate` on head (X) and body (X/Y).
#[must_use]
pub fn delimited_to_html(raw: &str, path: &str) -> String {
    let total_hint = total_rows_hint_from_raw(raw);
    match parse_csv(raw, Some(path)) {
        Ok(rows) if rows.is_empty() => format!("<pre>{}</pre>", html_escape_minimal(raw)),
        Ok(rows) => {
            let total_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
            let wide_note = (!should_render_as_table(&rows)).then(|| {
                format!(
                    "[wide delimited view: {total_cols} columns — scroll horizontally to see all]"
                )
            });
            rows_to_html_table(&rows, total_hint, wide_note.as_deref())
        }
        Err(_) => format!("<pre>{}</pre>", html_escape_minimal(raw)),
    }
}

/// Must stay in sync with `.csv-viewer .kv-table th, td { max-width: 36ch }` in ublx-web styles.
const WEB_CELL_ELLIPSIS_CHARS: usize = 36;

const SHELL_OPEN: &str = concat!(
    r#"<div class="csv-viewer__shell">"#,
    r#"<div class="csv-viewer__hbar" aria-hidden="true"><div class="csv-viewer__hspacer"></div></div>"#,
    r#"<div class="csv-viewer__corner" aria-hidden="true"></div>"#,
    r#"<div class="csv-viewer__viewport">"#,
    r#"<div class="csv-viewer__head"><div class="csv-viewer__head-inner">"#,
    r#"<table class="kv-table kv-table--wide"><thead><tr>"#,
);

const SHELL_MID: &str = concat!(
    "</tr></thead></table></div></div>",
    r#"<div class="csv-viewer__body"><div class="csv-viewer__inner">"#,
    r#"<table class="kv-table kv-table--wide"><tbody>"#,
);

const SHELL_TAIL: &str = concat!(
    "</tbody></table></div></div></div>",
    r#"<div class="csv-viewer__vbar" aria-hidden="true"><div class="csv-viewer__vspacer"></div></div>"#,
    "</div>",
);

fn rows_to_html_table(
    rows: &[Vec<String>],
    total_rows_hint: Option<usize>,
    wide_note: Option<&str>,
) -> String {
    let mut out = String::new();
    if let Some(note) = wide_note {
        let _ = write!(
            out,
            r#"<p class="csv-viewer__note">{}</p>"#,
            html_escape_minimal(note)
        );
    }
    if let Some(total) = total_rows_hint.filter(|t| *t > rows.len()) {
        let shown = rows.len();
        let hidden = total.saturating_sub(shown);
        let _ = write!(
            out,
            r#"<p class="csv-viewer__note">[delimited preview: {shown} rows shown, {hidden} hidden of {total}]</p>"#
        );
    }
    out.push_str(SHELL_OPEN);
    let (header, body) = rows
        .split_first()
        .map_or((&[][..], rows), |(h, b)| (h.as_slice(), b));
    if header.is_empty() {
        out.push_str(SHELL_MID);
        out.push_str(SHELL_TAIL);
        return out;
    }
    for cell in header {
        write_html_cell(&mut out, "th", cell);
    }
    out.push_str(SHELL_MID);
    for row in body {
        out.push_str("<tr>");
        let cols = header.len().max(row.len());
        for i in 0..cols {
            write_html_cell(&mut out, "td", row.get(i).map_or("", String::as_str));
        }
        out.push_str("</tr>");
    }
    out.push_str(SHELL_TAIL);
    out
}

fn write_html_cell(out: &mut String, tag: &str, cell: &str) {
    let text = pretty_tables::collapse_viewer_cell_whitespace(cell);
    let tip = if text.chars().count() > WEB_CELL_ELLIPSIS_CHARS {
        format!(r#" data-tip="{}""#, html_escape_minimal(&text))
    } else {
        String::new()
    };
    let _ = write!(out, "<{tag}{tip}>{}</{tag}>", html_escape_minimal(&text));
}
