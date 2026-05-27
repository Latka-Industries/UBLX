//! Section types and JSON parsing into Key/Value and Contents sections.

use rayon::prelude::*;
use serde_json::Value;

use crate::config::{ColumnStatsDisplay, PARALLEL};
use crate::ui::UI_STRINGS;

use super::column_metadata;
use super::consts::TABLE_GAP;
use super::format;
use super::parse_ctx::KvParseCtx;
use super::walk;

/// One key/value section: optional title and rows (key, value).
/// When [`sub_title`](KvSection::sub_title) is true, the title uses the same subordinate style as Contents sub-sections.
pub struct KvSection {
    pub title: Option<String>,
    pub rows: Vec<(String, String)>,
    pub sub_title: bool,
}

/// Multi-column table section (e.g. zip entries "Contents"). Stores raw entries for virtualization; only visible rows are built when drawing.
/// When [`sub_title`](ContentsSection::sub_title) is true, the title is drawn with a subordinate style (e.g. "`TableName` · Columns" under that table).
pub struct ContentsSection {
    pub title: String,
    pub columns: Vec<String>,
    pub column_keys: Vec<String>,
    pub entries: Vec<Value>,
    /// If true, title uses sub-section style (dimmer) to show it belongs under the previous section.
    pub sub_title: bool,
}

/// Single-column list with no header (e.g. `common_pivots`, schema tree).
pub struct SingleColumnListSection {
    pub title: String,
    pub values: Vec<String>,
}

/// Either a key/value table, a multi-column contents table, or a single-column list.
pub enum Section {
    KeyValue(KvSection),
    Contents(ContentsSection),
    SingleColumnList(SingleColumnListSection),
}

impl Section {
    /// Title string for the section, if any (used for drawing).
    #[must_use]
    pub fn title_str(&self) -> Option<&str> {
        match self {
            Section::KeyValue(kv) => kv.title.as_deref(),
            Section::Contents(c) => Some(c.title.as_str()),
            Section::SingleColumnList(list) => Some(list.title.as_str()),
        }
    }

    /// (`has_title`, `header_lines`, `num_rows`) for line counting and viewport math.
    #[must_use]
    pub fn line_metrics(&self) -> (bool, u16, usize) {
        match self {
            Section::KeyValue(kv) => (kv.title.is_some(), 1, kv.rows.len()),
            Section::Contents(c) => (true, 1, c.entries.len()),
            Section::SingleColumnList(list) => (true, 0, list.values.len()),
        }
    }

    /// True if the section title should use subordinate (dimmer) style.
    #[must_use]
    pub fn sub_title_style(&self) -> bool {
        match self {
            Section::KeyValue(kv) => kv.sub_title,
            Section::Contents(c) => c.sub_title,
            Section::SingleColumnList(_) => false,
        }
    }
}

/// Parse one blob into sections (either `csv_metadata` or walk). Returns empty vec on parse failure.
fn parse_one_blob(blob: &str, ctx: KvParseCtx) -> Vec<Section> {
    let value: Value = match serde_json::from_str(blob.trim()) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let Some(map) = value.as_object() else {
        return vec![];
    };
    let map = map
        .get("_metadata")
        .and_then(Value::as_object)
        .unwrap_or(map);
    if column_metadata::is_compact_column_metadata(map) {
        column_metadata::sections_from_column_metadata_root(
            map,
            ctx.max_array_inline,
            ctx.column_stats,
        )
    } else if column_metadata::is_legacy_parallel_column_metadata(map) {
        column_metadata::sections_from_legacy_column_metadata_root(map)
    } else {
        walk::root_parts_sections(map, ctx.max_array_inline, ctx.column_stats)
    }
}

/// Parse JSON string (one or more objects joined by "\n\n") into sections. First section is titled "General"; nested objects become separate sections; objects with "entries" get an extra "Contents" table. Special keys: schema (tree), `sheet_stats`, `common_pivots`, `csv_metadata`.
/// Uses parallel iteration when blob count exceeds [`PARALLEL.json_sections_blobs`].
/// For width-aware array display, use [`parse_json_sections_with`].
#[must_use]
pub fn parse_json_sections(json: &str) -> Vec<Section> {
    parse_json_sections_with(json, format::DEFAULT_MAX_ARRAY_INLINE, ColumnStatsDisplay::default())
}

/// Like [`parse_json_sections`], with `max_array_inline` passed through to value formatting (e.g. `shape` arrays).
#[must_use]
pub fn parse_json_sections_with(
    json: &str,
    max_array_inline: usize,
    column_stats: ColumnStatsDisplay,
) -> Vec<Section> {
    parse_json_sections_with_ctx(json, KvParseCtx::new(max_array_inline, column_stats))
}

/// Like [`parse_json_sections_with`], with an explicit [`KvParseCtx`].
#[must_use]
pub fn parse_json_sections_with_ctx(json: &str, ctx: KvParseCtx) -> Vec<Section> {
    let blobs: Vec<&str> = json
        .split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .collect();

    let mut sections: Vec<Section> = if blobs.len() >= PARALLEL.json_sections_blobs {
        blobs
            .par_iter()
            .flat_map(|blob| parse_one_blob(blob, ctx))
            .collect()
    } else {
        blobs
            .iter()
            .flat_map(|blob| parse_one_blob(blob, ctx))
            .collect()
    };

    if let Some(Section::KeyValue(kv)) = sections.first_mut() {
        kv.title = Some(UI_STRINGS.tables.first_title.to_string());
    }
    sections
}

/// One logical line per rendered row (same order and count as [`content_height`]). Find `n`/`N`
/// scroll uses newline indices in the joined string as `preview_scroll` coordinates.
#[must_use]
pub fn visual_lines_from_sections(sections: &[Section], max_array_inline: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            for _ in 0..TABLE_GAP {
                lines.push(String::new());
            }
        }
        match section {
            Section::KeyValue(kv) => {
                if let Some(t) = &kv.title {
                    lines.push(t.clone());
                }
                lines.push(format!(
                    "{} {}",
                    UI_STRINGS.tables.header_key, UI_STRINGS.tables.header_value
                ));
                for (k, v) in &kv.rows {
                    lines.push(format!("{k} {v}"));
                }
            }
            Section::Contents(c) => {
                lines.push(c.title.clone());
                lines.push(c.columns.join(" "));
                for entry in &c.entries {
                    if let Some(obj) = entry.as_object() {
                        let row: Vec<String> = c
                            .column_keys
                            .iter()
                            .map(|k| {
                                obj.get(k).map_or_else(
                                    || "—".to_string(),
                                    |v| format::format_value(v, k, max_array_inline),
                                )
                            })
                            .collect();
                        lines.push(row.join(" "));
                    }
                }
            }
            Section::SingleColumnList(list) => {
                lines.push(list.title.clone());
                for s in &list.values {
                    lines.push(s.clone());
                }
            }
        }
    }
    lines
}

/// Byte offset of each line start in `s` (including 0). Length equals newline count + 1.
/// Aligns with [`visual_lines_from_sections`] joined by `\n` (no trailing newline).
#[must_use]
pub fn line_byte_starts(s: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, b) in s.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

/// Haystack for in-pane find: tab `·n`, `cur/total`, `sync` ranges, and `n`/`N` scroll (must match
/// [`content_height`] line layout). Uses a fixed array inline cap; for draw alignment use [`searchable_text_from_json_with`].
#[must_use]
pub fn searchable_text_from_json(json: &str) -> String {
    searchable_text_from_json_with(json, format::DEFAULT_MAX_ARRAY_INLINE, ColumnStatsDisplay::default())
}

/// Same as [`searchable_text_from_json`], with `max_array_inline` matching [`parse_json_sections_with`].
#[must_use]
pub fn searchable_text_from_json_with(
    json: &str,
    max_array_inline: usize,
    column_stats: ColumnStatsDisplay,
) -> String {
    searchable_text_from_json_with_ctx(json, KvParseCtx::new(max_array_inline, column_stats))
}

/// Same as [`searchable_text_from_json_with`], with an explicit [`KvParseCtx`].
#[must_use]
pub fn searchable_text_from_json_with_ctx(json: &str, ctx: KvParseCtx) -> String {
    let sections = parse_json_sections_with_ctx(json, ctx);
    if sections.is_empty() {
        return json.to_string();
    }
    let lines = visual_lines_from_sections(&sections, ctx.max_array_inline);
    lines.join("\n")
}

/// Total line count for the parsed sections (title + header + data rows + gaps). Used for scrollbar and clamping.
#[must_use]
pub fn content_height(json: &str, column_stats: ColumnStatsDisplay) -> u16 {
    content_height_with_ctx(json, KvParseCtx::new(format::DEFAULT_MAX_ARRAY_INLINE, column_stats))
}

/// Like [`content_height`], with an explicit [`KvParseCtx`].
#[must_use]
pub fn content_height_with_ctx(json: &str, ctx: KvParseCtx) -> u16 {
    let sections = parse_json_sections_with_ctx(json, ctx);
    if sections.is_empty() {
        return 0;
    }
    let mut lines: u16 = 0;
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            lines += TABLE_GAP;
        }
        let (has_title, header_lines, num_rows) = section.line_metrics();
        lines += u16::from(has_title);
        lines += header_lines;
        lines += num_rows as u16;
    }
    lines
}
