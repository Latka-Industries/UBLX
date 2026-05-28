//! Zahir **column metadata**: one table per inferred type (string, date, boolean, number, …).
//!
//! Only the compact **`columns`** array is supported: each element has short keys (`i`, `name`, `t`,
//! `null_pct`, `uniq`) and optional nested `date` / `num` / `bool`. Older snapshots that still use
//! parallel `column_names` / `column_types` arrays show a notice instead of tables.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashSet};

use crate::config::ColumnStatsDisplay;

use super::format;
use super::sections::{ContentsSection, KvSection, Section};

/// Compact column array on root metadata and elsewhere (`npz_metadata.columns`, …).
const COMPACT_COLUMNS_KEY: &str = "columns";
/// Short keys inside each compact column object.
const COMPACT_NAME: &str = "name";
const COMPACT_TYPE: &str = "t";
const COMPACT_NULL_PCT: &str = "null_pct";
const COMPACT_UNIQUE: &str = "uniq";
const COMPACT_DATE: &str = "date";
const COMPACT_NUM: &str = "num";
const COMPACT_BOOL: &str = "bool";
/// Zahir compact boolean stat (parallel to legacy `true_percentage` on bool stats objects).
const COMPACT_BOOL_TRUE_PCT: &str = "true_pct";

/// Keys inside `date_stats` objects (`span_days`, min, max).
struct DateStatsKeys;
impl DateStatsKeys {
    const SPAN_DAYS: &'static str = "span_days";
    const MIN: &'static str = "min";
    const MAX: &'static str = "max";
}

/// Internal row key for true % (from compact `bool` after normalizing `true_pct`).
const BOOLEAN_STATS_TRUE_PCT: &str = "true_percentage";

/// Shown when stored JSON still uses parallel `column_names` / `column_types` (pre-compact Zahir).
const LEGACY_COLUMN_METADATA_MSG: &str =
    "Column stats use an old Zahir JSON shape. Remove cached ublx data and restart ublx.";
const ROOT_METADATA_HINT_KEY: &str = "_metadata";
const ROOT_METADATA_FALLBACK_TITLE: &str = "metadata";

/// Column type from parallel `column_types` (legacy) or compact `t`; drives which table we build.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ColumnType {
    String,
    Date,
    Boolean,
    Other,
}

impl ColumnType {
    fn from_type_str(s: &str) -> Self {
        match s {
            "string" => Self::String,
            // Logical timestamps use `t`: `timestamp`; stats still use the `date` object in compact JSON.
            "date" | "timestamp" => Self::Date,
            "boolean" => Self::Boolean,
            _ => Self::Other,
        }
    }

    /// `zahir_t` is the raw `t` field (`date` vs `timestamp`) for the section title.
    fn section_title(self, zahir_t: &str) -> String {
        match self {
            Self::String => "String columns".to_string(),
            Self::Date => {
                if zahir_t == "timestamp" {
                    "Timestamp columns".to_string()
                } else {
                    "Date columns".to_string()
                }
            }
            Self::Boolean => "Boolean columns".to_string(),
            Self::Other => format!("{} columns", format::format_key(zahir_t)),
        }
    }
}

const TYPE_UNKNOWN: &str = "unknown";

/// [`ColumnStatsDisplay::Abbrev`]: typed column tables longer than this show only [`ABBREV_TABLE_MAX_ROWS`].
const ABBREV_TABLE_ROW_THRESHOLD: usize = 20;
/// [`ColumnStatsDisplay::Abbrev`]: max data rows when over [`ABBREV_TABLE_ROW_THRESHOLD`].
const ABBREV_TABLE_MAX_ROWS: usize = 20;

fn abbrev_cap_entries(entries: Vec<Value>, mode: ColumnStatsDisplay) -> (Vec<Value>, bool) {
    if mode != ColumnStatsDisplay::Abbrev || entries.len() <= ABBREV_TABLE_ROW_THRESHOLD {
        return (entries, false);
    }
    (
        entries.into_iter().take(ABBREV_TABLE_MAX_ROWS).collect(),
        true,
    )
}

/// KV rows for per-column nested stats under sheet-style `columns` lists.
#[must_use]
pub fn nested_stats_kv_rows(
    stats_key: &str,
    stats_obj: &Map<String, Value>,
    max_array_inline: usize,
    mode: ColumnStatsDisplay,
) -> Vec<(String, String)> {
    let _ = stats_key;
    if !mode.shows_tables() {
        return vec![];
    }
    stats_object_to_kv_rows(stats_obj, max_array_inline)
}

fn stats_object_to_kv_rows(
    obj: &Map<String, Value>,
    max_array_inline: usize,
) -> Vec<(String, String)> {
    obj.iter()
        .map(|(k, val)| {
            (
                format::format_key(k),
                format::format_value(val, k, max_array_inline),
            )
        })
        .collect()
}

/// Keys inside `date_stats` objects for typed column tables.
fn date_stat_column_keys() -> [String; 3] {
    [
        DateStatsKeys::SPAN_DAYS.to_string(),
        DateStatsKeys::MIN.to_string(),
        DateStatsKeys::MAX.to_string(),
    ]
}

/// JSON keys used **inside rendered table rows** (from compact stats); parallel arrays at the top level are no longer ingested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataArrayKey {
    ColumnNames,
    ColumnTypes,
    NullPercentages,
    UniqueCounts,
    NumericStats,
    DateStats,
    BooleanStats,
}

impl MetadataArrayKey {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ColumnNames => "column_names",
            Self::ColumnTypes => "column_types",
            Self::NullPercentages => "null_percentages",
            Self::UniqueCounts => "unique_counts",
            Self::NumericStats => "numeric_stats",
            Self::DateStats => "date_stats",
            Self::BooleanStats => "boolean_stats",
        }
    }
}

/// Display label for a stat table column key (overrides for brevity).
fn stat_column_header_label(key: &str) -> String {
    match key {
        k if k == MetadataArrayKey::ColumnNames.as_str() => "Column".to_string(),
        k if k == MetadataArrayKey::UniqueCounts.as_str() => "Unique #".to_string(),
        k if k == MetadataArrayKey::NullPercentages.as_str() => "Null %".to_string(),
        k if k == BOOLEAN_STATS_TRUE_PCT || k == COMPACT_BOOL_TRUE_PCT => "True %".to_string(),
        _ => format::format_key(key),
    }
}

fn stat_column_headers_display(column_keys: &[String]) -> Vec<String> {
    column_keys
        .iter()
        .map(|k| stat_column_header_label(k))
        .collect()
}

fn contents_section(
    title: String,
    column_keys: Vec<String>,
    entries: Vec<Value>,
    mode: ColumnStatsDisplay,
) -> Option<ContentsSection> {
    if entries.is_empty() {
        return None;
    }
    let total = entries.len();
    let (entries, truncated) = abbrev_cap_entries(entries, mode);
    let title = if truncated {
        format!("{title} ({}/{total})", entries.len())
    } else {
        title
    };
    Some(ContentsSection {
        title,
        columns: stat_column_headers_display(&column_keys),
        column_keys,
        entries,
        sub_title: false,
    })
}

/// True if `arr` is Zahir’s compact column list: `{ name, t, … }` per element.
#[must_use]
pub fn is_compact_column_stats_array(arr: &[Value]) -> bool {
    let Some(first) = arr.first().and_then(Value::as_object) else {
        return false;
    };
    if first.get(COMPACT_NAME).and_then(Value::as_str).is_none()
        || first.get(COMPACT_TYPE).and_then(Value::as_str).is_none()
    {
        return false;
    }
    arr.iter().all(Value::is_object)
}

/// True if `obj` has a supported compact `columns` array (current Zahir).
#[must_use]
pub fn is_compact_column_metadata(obj: &Map<String, Value>) -> bool {
    obj.get(COMPACT_COLUMNS_KEY)
        .and_then(Value::as_array)
        .is_some_and(|cols| is_compact_column_stats_array(cols))
}

/// True if `obj` still has parallel `column_names` / `column_types` and no valid compact `columns` (old Zahir / stale DB).
#[must_use]
pub fn is_legacy_parallel_column_metadata(obj: &Map<String, Value>) -> bool {
    if is_compact_column_metadata(obj) {
        return false;
    }
    let names = obj
        .get(MetadataArrayKey::ColumnNames.as_str())
        .and_then(Value::as_array);
    let types = obj
        .get(MetadataArrayKey::ColumnTypes.as_str())
        .and_then(Value::as_array);
    matches!((names, types), (Some(n), Some(t)) if n.len() == t.len())
}

/// Typed tables from a compact `columns` array (same layout as legacy parallel-array metadata).
/// `title_prefix`: when set (e.g. nested table name), section titles become `prefix · String columns`, etc.
#[must_use]
pub fn typed_sections_from_compact_columns(
    columns: &[Value],
    title_prefix: Option<&str>,
    mode: ColumnStatsDisplay,
) -> Vec<Section> {
    if !is_compact_column_stats_array(columns) {
        return vec![];
    }
    let mut cols: Vec<Value> = columns.to_vec();
    sort_compact_columns_by_index(&mut cols);
    let Some(parallel) = compact_columns_to_parallel_arrays(&cols) else {
        return vec![];
    };
    let mut out = parallel_arrays_to_sections(&parallel, mode);
    if let Some(prefix) = title_prefix.filter(|s| !s.is_empty()) {
        for section in &mut out {
            if let Section::Contents(c) = section {
                c.title = format::join_dot([prefix, c.title.as_str()]);
                c.sub_title = true;
            }
        }
    }
    out
}

fn sort_compact_columns_by_index(columns: &mut [Value]) {
    let all_indexed = columns.iter().all(|v| {
        v.as_object()
            .and_then(|o| o.get("i"))
            .is_some_and(serde_json::Value::is_number)
    });
    if !all_indexed {
        return;
    }
    columns.sort_by(|a, b| {
        let ai = json_number_sort_key(a.as_object().and_then(|o| o.get("i")));
        let bi = json_number_sort_key(b.as_object().and_then(|o| o.get("i")));
        ai.cmp(&bi)
    });
}

fn json_number_sort_key(v: Option<&Value>) -> i64 {
    v.and_then(serde_json::Value::as_f64)
        .map_or(0, |f| f as i64)
}

/// Parallel per-column arrays built from a compact `columns` list (indices aligned).
/// `null_pct` / `unique` use [`Option`]: [`None`] means the key was omitted in JSON for that column.
struct ParallelColumnArrays {
    names: Vec<Value>,
    types: Vec<Value>,
    null_pct: Vec<Option<Value>>,
    unique: Vec<Option<Value>>,
    date_stats: Vec<Value>,
    bool_stats: Vec<Value>,
    num_stats: Vec<Value>,
}

/// Converts compact column objects into [`ParallelColumnArrays`] for [`parallel_arrays_to_sections`].
fn compact_columns_to_parallel_arrays(columns: &[Value]) -> Option<ParallelColumnArrays> {
    let mut names = Vec::with_capacity(columns.len());
    let mut types = Vec::with_capacity(columns.len());
    let mut null_pct = Vec::with_capacity(columns.len());
    let mut unique = Vec::with_capacity(columns.len());
    let mut date_stats = Vec::with_capacity(columns.len());
    let mut bool_stats = Vec::with_capacity(columns.len());
    let mut num_stats = Vec::with_capacity(columns.len());

    for v in columns {
        let obj = v.as_object()?;
        names.push(obj.get(COMPACT_NAME).cloned().unwrap_or(Value::Null));
        types.push(obj.get(COMPACT_TYPE).cloned().unwrap_or(Value::Null));
        null_pct.push(obj.get(COMPACT_NULL_PCT).cloned());
        unique.push(obj.get(COMPACT_UNIQUE).cloned());
        date_stats.push(obj.get(COMPACT_DATE).cloned().unwrap_or(Value::Null));

        let bool_cell = match obj.get(COMPACT_BOOL).and_then(Value::as_object) {
            Some(b) => {
                let mut m = Map::new();
                if let Some(tp) = b.get(COMPACT_BOOL_TRUE_PCT) {
                    m.insert(BOOLEAN_STATS_TRUE_PCT.to_string(), tp.clone());
                }
                Value::Object(m)
            }
            None => Value::Null,
        };
        bool_stats.push(bool_cell);

        num_stats.push(obj.get(COMPACT_NUM).cloned().unwrap_or(Value::Null));
    }
    Some(ParallelColumnArrays {
        names,
        types,
        null_pct,
        unique,
        date_stats,
        bool_stats,
        num_stats,
    })
}

/// Build one table per column type from compact `columns` only.
/// `table_title` is the parent section’s display title: typed tables get `table_title · String columns`, etc. ([`typed_sections_from_compact_columns`]).
pub fn column_metadata_to_sections(
    map: &Map<String, Value>,
    table_title: Option<&str>,
    mode: ColumnStatsDisplay,
) -> Vec<Section> {
    let Some(cols) = map.get(COMPACT_COLUMNS_KEY).and_then(Value::as_array) else {
        return vec![];
    };
    if !is_compact_column_stats_array(cols) {
        return vec![];
    }
    typed_sections_from_compact_columns(cols, table_title.filter(|s| !s.is_empty()), mode)
}

fn parallel_arrays_to_sections(
    parallel: &ParallelColumnArrays,
    mode: ColumnStatsDisplay,
) -> Vec<Section> {
    if !mode.shows_tables() {
        return vec![];
    }
    let ParallelColumnArrays {
        names,
        types,
        null_pct,
        unique: uniq,
        date_stats,
        bool_stats,
        num_stats,
    } = parallel;
    let mut by_type: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, t) in types.iter().enumerate() {
        let key = t.as_str().unwrap_or(TYPE_UNKNOWN).to_string();
        by_type.entry(key).or_default().push(i);
    }

    let mut out = Vec::new();
    for (type_name, indices) in by_type {
        let col_type = ColumnType::from_type_str(&type_name);
        let section = match col_type {
            ColumnType::String => table_string(names, null_pct, uniq, indices, mode),
            ColumnType::Date => table_date(
                names,
                null_pct,
                date_stats,
                indices,
                type_name.as_str(),
                mode,
            ),
            ColumnType::Boolean => table_boolean(names, null_pct, bool_stats, indices, mode),
            ColumnType::Other => {
                table_numeric_or_other(&type_name, names, null_pct, num_stats, indices, mode)
            }
        };
        if let Some(s) = section {
            out.push(Section::Contents(s));
        }
    }
    out
}

#[inline]
fn any_row_has_field(indices: &[usize], field: &[Option<Value>]) -> bool {
    indices
        .iter()
        .any(|&i| field.get(i).is_some_and(std::option::Option::is_some))
}

fn push_column_name_row(row: &mut Map<String, Value>, names: &[Value], i: usize) {
    row.insert(
        MetadataArrayKey::ColumnNames.as_str().to_string(),
        names.get(i).cloned().unwrap_or(Value::Null),
    );
}

/// If `show_null_col` is true, insert null % when present for this row (omit key if absent → "—" in UI).
fn push_null_pct_cell(
    row: &mut Map<String, Value>,
    null_pct: &[Option<Value>],
    i: usize,
    show_null_col: bool,
) {
    if !show_null_col {
        return;
    }
    if let Some(v) = null_pct.get(i).and_then(|x| x.as_ref()) {
        row.insert(
            MetadataArrayKey::NullPercentages.as_str().to_string(),
            v.clone(),
        );
    }
}

/// If `show_uniq_col` is true, insert uniq when present for this row.
fn push_uniq_cell(
    row: &mut Map<String, Value>,
    uniq: &[Option<Value>],
    i: usize,
    show_uniq_col: bool,
) {
    if !show_uniq_col {
        return;
    }
    if let Some(v) = uniq.get(i).and_then(|x| x.as_ref()) {
        row.insert(
            MetadataArrayKey::UniqueCounts.as_str().to_string(),
            v.clone(),
        );
    }
}

fn table_string(
    names: &[Value],
    null_pct: &[Option<Value>],
    uniq: &[Option<Value>],
    indices: Vec<usize>,
    mode: ColumnStatsDisplay,
) -> Option<ContentsSection> {
    let show_null = any_row_has_field(&indices, null_pct);
    let show_uniq = any_row_has_field(&indices, uniq);
    let mut column_keys = vec![MetadataArrayKey::ColumnNames.as_str().to_string()];
    if show_null {
        column_keys.push(MetadataArrayKey::NullPercentages.as_str().to_string());
    }
    if show_uniq {
        column_keys.push(MetadataArrayKey::UniqueCounts.as_str().to_string());
    }
    let entries: Vec<Value> = indices
        .into_iter()
        .map(|i| {
            let mut row = Map::new();
            push_column_name_row(&mut row, names, i);
            push_null_pct_cell(&mut row, null_pct, i, show_null);
            push_uniq_cell(&mut row, uniq, i, show_uniq);
            Value::Object(row)
        })
        .collect();
    contents_section(
        ColumnType::String.section_title("string"),
        column_keys,
        entries,
        mode,
    )
}

fn column_keys_typed_with_null(
    show_null: bool,
    extra: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut keys = vec![MetadataArrayKey::ColumnNames.as_str().to_string()];
    if show_null {
        keys.push(MetadataArrayKey::NullPercentages.as_str().to_string());
    }
    keys.extend(extra);
    keys
}

fn table_date(
    names: &[Value],
    null_pct: &[Option<Value>],
    date_stats: &[Value],
    indices: Vec<usize>,
    zahir_t: &str,
    mode: ColumnStatsDisplay,
) -> Option<ContentsSection> {
    let show_null = any_row_has_field(&indices, null_pct);
    let column_keys = column_keys_typed_with_null(show_null, date_stat_column_keys());
    let entries: Vec<Value> = indices
        .into_iter()
        .map(|i| {
            let mut row = Map::new();
            push_column_name_row(&mut row, names, i);
            push_null_pct_cell(&mut row, null_pct, i, show_null);
            let stats = date_stats.get(i).and_then(Value::as_object);
            for key in column_keys.iter().filter(|k| {
                *k != MetadataArrayKey::ColumnNames.as_str()
                    && *k != MetadataArrayKey::NullPercentages.as_str()
            }) {
                let val = stats
                    .and_then(|s| s.get(key.as_str()))
                    .cloned()
                    .unwrap_or(Value::Null);
                row.insert(key.clone(), val);
            }
            Value::Object(row)
        })
        .collect();
    contents_section(
        ColumnType::Date.section_title(zahir_t),
        column_keys,
        entries,
        mode,
    )
}

fn table_boolean(
    names: &[Value],
    null_pct: &[Option<Value>],
    bool_stats: &[Value],
    indices: Vec<usize>,
    mode: ColumnStatsDisplay,
) -> Option<ContentsSection> {
    let show_null = any_row_has_field(&indices, null_pct);
    let column_keys = column_keys_typed_with_null(show_null, [BOOLEAN_STATS_TRUE_PCT.to_string()]);
    let entries: Vec<Value> = indices
        .into_iter()
        .map(|i| {
            let mut row = Map::new();
            push_column_name_row(&mut row, names, i);
            push_null_pct_cell(&mut row, null_pct, i, show_null);
            let pct = bool_stats.get(i).and_then(Value::as_object).and_then(|o| {
                o.get(BOOLEAN_STATS_TRUE_PCT)
                    .cloned()
                    .or_else(|| o.get(COMPACT_BOOL_TRUE_PCT).cloned())
            });
            row.insert(
                BOOLEAN_STATS_TRUE_PCT.to_string(),
                pct.unwrap_or(Value::Null),
            );
            Value::Object(row)
        })
        .collect();
    contents_section(
        ColumnType::Boolean.section_title("boolean"),
        column_keys,
        entries,
        mode,
    )
}

/// Collect `numeric_stats` keys in JSON order: first object’s keys in order, then any keys from other objects not yet seen.
fn numeric_stats_keys(num_stats: &[Value]) -> Vec<String> {
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    for v in num_stats.iter().filter_map(Value::as_object) {
        for k in v.keys() {
            if seen.insert(k.clone()) {
                order.push(k.clone());
            }
        }
    }
    order
}

fn table_numeric_or_other(
    type_name: &str,
    names: &[Value],
    null_pct: &[Option<Value>],
    num_stats: &[Value],
    indices: Vec<usize>,
    mode: ColumnStatsDisplay,
) -> Option<ContentsSection> {
    let show_null = any_row_has_field(&indices, null_pct);
    let stat_keys = numeric_stats_keys(num_stats);
    let column_keys = column_keys_typed_with_null(show_null, stat_keys.clone());
    let entries: Vec<Value> = indices
        .into_iter()
        .map(|i| {
            let mut row = Map::new();
            push_column_name_row(&mut row, names, i);
            push_null_pct_cell(&mut row, null_pct, i, show_null);
            let stats = num_stats.get(i).and_then(Value::as_object);
            for k in &stat_keys {
                let val = stats.and_then(|s| s.get(k).cloned()).unwrap_or(Value::Null);
                row.insert(k.clone(), val);
            }
            Value::Object(row)
        })
        .collect();
    contents_section(
        ColumnType::Other.section_title(type_name),
        column_keys,
        entries,
        mode,
    )
}

fn flat_kv_rows_for_column_metadata(
    metadata: &Map<String, Value>,
    max_array_inline: usize,
) -> Vec<(String, String)> {
    metadata
        .iter()
        .filter(|(key, _)| key.as_str() != COMPACT_COLUMNS_KEY)
        .map(|(key, val)| {
            (
                format::format_key(key),
                format::format_value(val, key, max_array_inline),
            )
        })
        .collect()
}

/// Push flat KV for non-table fields (if any), then typed column tables.
fn push_column_metadata_flat_kv_and_tables(
    sections: &mut Vec<Section>,
    title: Option<String>,
    metadata: &Map<String, Value>,
    max_array_inline: usize,
    mode: ColumnStatsDisplay,
) {
    let table_title = title.clone();
    let flat_kv = flat_kv_rows_for_column_metadata(metadata, max_array_inline);
    if !flat_kv.is_empty() {
        sections.push(Section::KeyValue(KvSection {
            title,
            rows: flat_kv,
            sub_title: false,
        }));
    }
    sections.extend(column_metadata_to_sections(
        metadata,
        table_title.as_deref().filter(|s| !s.is_empty()),
        mode,
    ));
}

/// Push flat KV for scalar fields, then typed tables from [`column_metadata_to_sections`].
/// When `display_title` is set, it is the section title; otherwise the title is [`format::format_key`]`(section_key)`.
pub fn push_column_metadata_sections(
    sections: &mut Vec<Section>,
    section_key: &str,
    metadata: &Map<String, Value>,
    max_array_inline: usize,
    display_title: Option<&str>,
    mode: ColumnStatsDisplay,
) {
    let title = display_title.map_or_else(|| format::format_key(section_key), str::to_string);
    push_column_metadata_flat_kv_and_tables(
        sections,
        Some(title),
        metadata,
        max_array_inline,
        mode,
    );
}

/// Root blob is entirely compact column metadata (flat KV for scalars + typed tables).
#[must_use]
pub fn sections_from_column_metadata_root(
    metadata: &Map<String, Value>,
    max_array_inline: usize,
    mode: ColumnStatsDisplay,
) -> Vec<Section> {
    let title = metadata
        .get(ROOT_METADATA_HINT_KEY)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(ROOT_METADATA_FALLBACK_TITLE);
    let mut out = Vec::new();
    push_column_metadata_flat_kv_and_tables(
        &mut out,
        Some(format::format_key(title)),
        metadata,
        max_array_inline,
        mode,
    );
    out
}

/// One KV section telling the user to drop stale DB / cache when parallel-array column JSON is still stored.
#[must_use]
pub fn sections_from_legacy_column_metadata_root(metadata: &Map<String, Value>) -> Vec<Section> {
    let title = metadata
        .get(ROOT_METADATA_HINT_KEY)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(ROOT_METADATA_FALLBACK_TITLE);
    let mut sections = Vec::new();
    push_legacy_column_metadata_notice(&mut sections, Some(format::format_key(title)), false);
    sections
}

/// Push a short notice instead of typed tables (nested or root callers pass `sub_title` when under another section).
pub fn push_legacy_column_metadata_notice(
    sections: &mut Vec<Section>,
    title: Option<String>,
    sub_title: bool,
) {
    sections.push(Section::KeyValue(KvSection {
        title,
        rows: vec![(
            "Column stats".to_string(),
            LEGACY_COLUMN_METADATA_MSG.to_string(),
        )],
        sub_title,
    }));
}
