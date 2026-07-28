//! Comfy-table (Unicode box drawing) for the file viewer: CSV, Markdown, and shared layout helpers.

use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};
#[cfg(feature = "tui")]
use ratatui::style::{Color, Style};
#[cfg(feature = "tui")]
use ratatui::text::{Line, Span, Text};
use rayon::prelude::*;

use crate::config::PARALLEL;
#[cfg(feature = "tui")]
use crate::themes;

// -----------------------------------------------------------------------------
// Truncation helpers (legacy explicit max, parallel path)
// -----------------------------------------------------------------------------

fn truncate_cell(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max).collect::<String>())
    }
}

/// Truncate all cells in parallel (when many rows) or sequentially.
fn truncate_all_cells(rows: &[Vec<String>], max_chars: usize) -> Vec<Vec<String>> {
    let truncate_row = |row: &Vec<String>| {
        row.iter()
            .map(|c| truncate_cell(c, max_chars))
            .collect::<Vec<_>>()
    };
    if rows.len() >= PARALLEL.csv_truncate {
        rows.par_iter().map(truncate_row).collect()
    } else {
        rows.iter().map(truncate_row).collect()
    }
}

/// Per-cell truncation cap derived from the viewer width and number of columns.
///
/// Comfy-table still receives `set_width(content_width)`; this limits how much text we keep per
/// cell before `"..."` so wide many-column tables don’t over‑truncate when the pane is narrow,
/// and single-column views can use more of a wide pane (up to `ceiling`).
#[must_use]
pub fn max_cell_chars_for_viewport(
    content_width: u16,
    col_count: usize,
    floor: usize,
    ceiling: usize,
) -> usize {
    let cols = col_count.max(1);
    let w = content_width as usize;
    if w == 0 {
        return ceiling;
    }
    let per_col = w / cols;
    // Borders / padding eat a few terminal columns per cell in UTF-8 box-draw mode.
    let fudge = 4usize;
    per_col.saturating_sub(fudge).clamp(floor, ceiling)
}

fn base_table(content_width: u16) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(content_width);
    table
}

// -----------------------------------------------------------------------------
// Smart wrap + ellipsis (CSV + markdown): proportional widths, short columns, padding
// -----------------------------------------------------------------------------

/// Columns whose **maximum** cell length (characters) is ≤ this use one line per cell (no word wrap).
/// Shorter cells in other columns still get blank lines so row heights align.
pub const VIEWER_TABLE_NO_WRAP_COL_MAX_CHARS: usize = 14;

/// Cells longer than this are shown as a single truncated line with [`truncate_cell`] / `"..."`.
pub const VIEWER_TABLE_ELLIPSIS_CELL_CHARS: usize = 512;

/// Collapse embedded newlines / whitespace (CSV cells with `\n`, markdown normalization).
#[must_use]
pub fn collapse_viewer_cell_whitespace(cell: &str) -> String {
    cell.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[must_use]
pub fn pad_row_to_cols(row: &[String], col_count: usize) -> Vec<String> {
    let mut r: Vec<String> = row.to_vec();
    if r.len() > col_count {
        r.truncate(col_count);
    }
    r.resize_with(col_count, String::new);
    r
}

/// Allocate per-column wrap widths: proportional to (capped) longest cell per column.
#[must_use]
pub fn column_wrap_widths(viewport: u16, col_count: usize, max_lens: &[usize]) -> Vec<usize> {
    const MIN_WRAP: usize = 4;
    let cols = col_count.max(1);
    let w = viewport as usize;
    if w <= 2 {
        return vec![MIN_WRAP; cols];
    }
    let border_overhead = 1usize.saturating_add(cols.saturating_mul(3));
    let inner = w.saturating_sub(border_overhead).max(cols * MIN_WRAP);
    let weights: Vec<usize> = max_lens.iter().copied().map(|m| m.max(1)).collect();
    let sum_w: usize = weights.iter().sum();
    if sum_w == 0 {
        return vec![MIN_WRAP; cols];
    }
    let mut widths: Vec<usize> = weights
        .iter()
        .map(|&wt| (inner * wt / sum_w).max(MIN_WRAP))
        .collect();
    let total: usize = widths.iter().sum();
    if total > inner {
        let mut excess = total - inner;
        while excess > 0 {
            let Some((i, _)) = widths.iter().enumerate().max_by_key(|(_, x)| *x) else {
                break;
            };
            if widths[i] > MIN_WRAP {
                widths[i] -= 1;
                excess -= 1;
            } else {
                break;
            }
        }
    } else if total < inner
        && let Some((i, _)) = widths.iter().enumerate().max_by_key(|(_, x)| *x)
    {
        widths[i] += inner - total;
    }
    widths
}

/// Outcome of [`prepare_cell_text_for_wrap`]: either final lines (ellipsis / short column) or text to run through wrap.
enum CellPrep {
    Done(Vec<String>),
    Wrap(String),
}

/// Shared whitespace wrap: `break_long` splits a single overlong whitespace-token (no spaces inside).
fn word_wrap_core(
    text: &str,
    max_chars: usize,
    break_long: impl Fn(&str, usize) -> Vec<String>,
) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return vec![String::new()];
    }
    if max_chars == 0 {
        return vec![text.to_string()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    let flush = |current: &mut String, lines: &mut Vec<String>| {
        if !current.is_empty() {
            lines.push(std::mem::take(current));
        }
    };

    for word in text.split_whitespace() {
        let wl = word.chars().count();
        let cur_len = current.chars().count();
        let with_space = usize::from(cur_len > 0);
        let needed = with_space + wl;

        if cur_len + needed <= max_chars {
            if with_space == 1 {
                current.push(' ');
            }
            current.push_str(word);
        } else {
            flush(&mut current, &mut lines);
            if wl > max_chars {
                let chunks = break_long(word, max_chars);
                let mut it = chunks.into_iter().peekable();
                while let Some(chunk) = it.next() {
                    if it.peek().is_some() {
                        lines.push(chunk);
                    } else {
                        current = chunk;
                    }
                }
            } else {
                current.push_str(word);
            }
        }
    }
    flush(&mut current, &mut lines);
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Word-wrap at whitespace; lines are at most `max_chars` Unicode scalar characters.
#[must_use]
pub fn word_wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    word_wrap_core(text, max_chars, hard_break_scalar_chunks)
}

/// Hard-break `text` into lines of at most `max_chars` Unicode scalars (no hyphen / space logic).
/// Linear in `text` length (single pass over chars).
fn hard_break_scalar_chunks(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut iter = text.chars();
    loop {
        let mut chunk = String::new();
        for _ in 0..max_chars {
            if let Some(c) = iter.next() {
                chunk.push(c);
            } else {
                break;
            }
        }
        if chunk.is_empty() {
            break;
        }
        lines.push(chunk);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Concatenate `tokens` without spaces, breaking lines so each line is at most `max_chars` scalars.
fn pack_concatenated_tokens(tokens: &[String], max_chars: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for t in tokens {
        let tl = t.chars().count();
        let cl = cur.chars().count();
        if tl > max_chars {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            lines.extend(hard_break_scalar_chunks(t, max_chars));
            continue;
        }
        if cur.is_empty() {
            cur.clone_from(t);
            continue;
        }
        if cl + tl <= max_chars {
            cur.push_str(t);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.clone_from(t);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Split `word` on `-` and pack as `part0`, `-part1`, `-part2`, … so line breaks prefer hyphens
/// (e.g. `2025-10-25` → `2025` / `-10` / `-25` instead of `2025` / `-10-` / `25`).
fn wrap_hyphenated_token(word: &str, max_chars: usize) -> Vec<String> {
    let parts: Vec<&str> = word.split('-').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 1 {
        return hard_break_scalar_chunks(word, max_chars);
    }
    let mut tokens: Vec<String> = Vec::with_capacity(parts.len());
    tokens.push(parts[0].to_string());
    for p in parts.iter().skip(1) {
        tokens.push(format!("-{p}"));
    }
    pack_concatenated_tokens(&tokens, max_chars)
}

/// Like [`word_wrap_text`], but treats `-` as a break opportunity inside long whitespace-tokens
/// (e.g. ISO dates in CSV cells).
#[must_use]
pub fn word_wrap_cell_text(text: &str, max_chars: usize) -> Vec<String> {
    word_wrap_core(text, max_chars, |word, m| {
        if word.contains('-') {
            wrap_hyphenated_token(word, m)
        } else {
            hard_break_scalar_chunks(word, m)
        }
    })
}

/// Collapsed cell text: either final [`CellPrep::Done`] lines (ellipsis / short column) or
/// [`CellPrep::Wrap`] for header/body wrap.
fn prepare_cell_text_for_wrap(
    text: &str,
    wrap_width: usize,
    column_max_content_chars: usize,
) -> CellPrep {
    let t = collapse_viewer_cell_whitespace(text);
    let len = t.chars().count();
    if len > VIEWER_TABLE_ELLIPSIS_CELL_CHARS {
        let cap = wrap_width.max(8);
        return CellPrep::Done(vec![truncate_cell(&t, cap)]);
    }
    if column_max_content_chars <= VIEWER_TABLE_NO_WRAP_COL_MAX_CHARS {
        return CellPrep::Done(if t.is_empty() {
            vec![String::new()]
        } else {
            vec![t]
        });
    }
    CellPrep::Wrap(t)
}

/// **Header / column name only:** one token (or whole title with no spaces). Tokens no longer than
/// [`VIEWER_TABLE_NO_WRAP_COL_MAX_CHARS`] stay on **one** line even when `wrap_width` is tiny
/// (e.g. `"Completion"` not sliced to 4); longer tokens use [`word_wrap_cell_text`].
fn wrap_header_name_token(word: &str, wrap_width: usize) -> Vec<String> {
    let wc = word.chars().count();
    if wc <= wrap_width || wc <= VIEWER_TABLE_NO_WRAP_COL_MAX_CHARS {
        vec![word.to_string()]
    } else {
        word_wrap_cell_text(word, wrap_width)
    }
}

/// Body cell lines. Short columns stay one line; otherwise [`word_wrap_cell_text`] on the whole cell
/// (whitespace-aware wrap inside that — no “one line per word” preprocessing).
fn cell_display_lines(
    text: &str,
    wrap_width: usize,
    column_max_content_chars: usize,
) -> Vec<String> {
    match prepare_cell_text_for_wrap(text, wrap_width, column_max_content_chars) {
        CellPrep::Done(lines) => lines,
        CellPrep::Wrap(t) => word_wrap_cell_text(&t, wrap_width),
    }
}

/// Column header cell: same early rules as body; then short tokens (≤ [`VIEWER_TABLE_NO_WRAP_COL_MAX_CHARS`])
/// are never hard-wrapped to `wrap_width` — only for **names**, not body cells.
fn header_cell_display_lines(
    text: &str,
    wrap_width: usize,
    column_max_content_chars: usize,
) -> Vec<String> {
    match prepare_cell_text_for_wrap(text, wrap_width, column_max_content_chars) {
        CellPrep::Done(lines) => lines,
        CellPrep::Wrap(t) => {
            if t.contains(' ') {
                let mut out: Vec<String> = Vec::new();
                for word in t.split_whitespace() {
                    if word.is_empty() {
                        continue;
                    }
                    out.extend(wrap_header_name_token(word, wrap_width));
                }
                if out.is_empty() {
                    out.push(String::new());
                }
                out
            } else {
                wrap_header_name_token(&t, wrap_width)
            }
        }
    }
}

fn pad_row_cells(wrapped: Vec<Vec<String>>, target_lines: usize) -> Vec<String> {
    wrapped
        .into_iter()
        .map(|mut lines| {
            while lines.len() < target_lines {
                lines.push(String::new());
            }
            lines.join("\n")
        })
        .collect()
}

/// Build header row and body rows as multiline cell strings for [`table_string_multiline`].
#[must_use]
pub fn prepare_multiline_grid(
    header: &[String],
    body: &[Vec<String>],
    content_width: u16,
) -> (Vec<String>, Vec<Vec<String>>) {
    let col_count = header
        .len()
        .max(body.iter().map(Vec::len).max().unwrap_or(0))
        .max(1);

    let header_cells = pad_row_to_cols(header, col_count);
    let body: Vec<Vec<String>> = body.iter().map(|r| pad_row_to_cols(r, col_count)).collect();

    let max_lens_at = |j: usize| -> usize {
        let h_len = collapse_viewer_cell_whitespace(header_cells.get(j).map_or("", String::as_str))
            .chars()
            .count();
        let b_len = body
            .iter()
            .map(|r| {
                collapse_viewer_cell_whitespace(r[j].as_str())
                    .chars()
                    .count()
            })
            .max()
            .unwrap_or(0);
        h_len.max(b_len)
    };

    let use_par_maxlens = col_count >= PARALLEL.pretty_tables_prep.maxlens_min_cols
        && body.len() >= PARALLEL.pretty_tables_prep.maxlens_min_body_rows;
    let max_lens: Vec<usize> = if use_par_maxlens {
        (0..col_count).into_par_iter().map(max_lens_at).collect()
    } else {
        (0..col_count).map(max_lens_at).collect()
    };

    let lens_for_widths: Vec<usize> = max_lens
        .iter()
        .copied()
        .map(|m| m.min(VIEWER_TABLE_ELLIPSIS_CELL_CHARS))
        .collect();

    let wrap_widths = column_wrap_widths(content_width, col_count, &lens_for_widths);

    let header_wrapped: Vec<Vec<String>> = header_cells
        .iter()
        .enumerate()
        .map(|(j, s)| header_cell_display_lines(s, wrap_widths[j], max_lens[j]))
        .collect();
    let header_h = header_wrapped.iter().map(Vec::len).max().unwrap_or(1);
    let comfy_header = pad_row_cells(header_wrapped, header_h);

    let process_body_row = |row: &Vec<String>| {
        let wrapped: Vec<Vec<String>> = row
            .iter()
            .enumerate()
            .map(|(j, s)| cell_display_lines(s, wrap_widths[j], max_lens[j]))
            .collect();
        let row_h = wrapped.iter().map(Vec::len).max().unwrap_or(1);
        pad_row_cells(wrapped, row_h)
    };

    let comfy_body: Vec<Vec<String>> = if body.len() >= PARALLEL.pretty_tables_prep.body_rows {
        body.par_iter().map(process_body_row).collect()
    } else {
        body.iter().map(process_body_row).collect()
    };

    (comfy_header, comfy_body)
}

// -----------------------------------------------------------------------------
// Public comfy-table string builders
// -----------------------------------------------------------------------------

/// Build a comfy-table string from parsed rows (first row as header), truncating cells.
#[must_use]
pub fn table_string_with_max_cell(
    rows: &[Vec<String>],
    content_width: u16,
    max_cell_chars: usize,
) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut truncated = truncate_all_cells(rows, max_cell_chars);
    let mut table = base_table(content_width);
    let header = truncated.remove(0);
    table.set_header(header);
    for row in truncated {
        table.add_row(row);
    }
    table.to_string()
}

/// CSV / markdown: proportional word wrap, short-column no-wrap, ellipsis beyond
/// [`VIEWER_TABLE_ELLIPSIS_CELL_CHARS`], then comfy-table.
#[must_use]
pub fn table_string_header_body_smart_wrap(
    header: &[String],
    body: &[Vec<String>],
    content_width: u16,
) -> String {
    let (h, b) = prepare_multiline_grid(header, body, content_width);
    table_string_multiline(h, &b, content_width)
}

/// Build a comfy-table from a header row and body rows without truncating. Cells may contain
/// `\n` for multi-line content; comfy-table expands row height and aligns columns.
#[must_use]
pub fn table_string_multiline(
    header: Vec<String>,
    body: &[Vec<String>],
    content_width: u16,
) -> String {
    let mut table = base_table(content_width);
    if !header.is_empty() {
        table.set_header(header);
    }
    for row in body {
        table.add_row(row.clone());
    }
    table.to_string()
}

/// Build a comfy-table string from parsed rows, treating all rows as body rows (no header).
#[must_use]
pub fn table_string_rows_only(
    rows: &[Vec<String>],
    content_width: u16,
    max_cell_chars: usize,
) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let truncated = truncate_all_cells(rows, max_cell_chars);
    let mut table = base_table(content_width);
    for row in truncated {
        table.add_row(row);
    }
    table.to_string()
}

/// Convert a pre-rendered table string into lines with the given foreground (e.g. from a captured
/// [`crate::themes::Palette`] when rendering off the UI thread or on Rayon workers).
#[cfg(feature = "tui")]
#[must_use]
pub fn table_string_to_lines_with_fg(table_str: &str, fg: Color) -> Vec<Line<'static>> {
    let style = Style::default().fg(fg);
    table_str
        .lines()
        .map(|l| Line::from(Span::styled(l.to_string(), style)))
        .collect()
}

/// Convert a pre-rendered table string into lines styled with [`themes::current`] text color.
#[cfg(feature = "tui")]
#[must_use]
pub fn table_string_to_lines(table_str: &str) -> Vec<Line<'static>> {
    table_string_to_lines_with_fg(table_str, themes::current().text)
}

/// Convert a pre-rendered table string into styled text.
#[cfg(feature = "tui")]
#[must_use]
pub fn table_string_to_text(table_str: &str) -> Text<'static> {
    Text::from(table_string_to_lines(table_str))
}
