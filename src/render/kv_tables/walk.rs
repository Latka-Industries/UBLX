//! Walk JSON metadata maps into [`Section`]s (Metadata / Writing tabs).
//!
//! ## Root
//! [`push_root_parts`] classifies each top-level key into [`RootBuckets`], then emits sections in a
//! fixed order (flat KV, schema, sheet stats, …). [`WalkKeyVars::FILE_TYPE`] is stripped first
//! ([`map_without_display_file_type`]) because the category column already shows the type.
//!
//! ## Nested maps and `entries` spill
//! [`process_nested_map`] handles arbitrary nested objects. Rows under [`SectionKeys::ENTRIES`] can
//! spill extra sections; child titles use `row_label · key` (see [`expand_object_children_with_prefix`]).
//!
//! ## Arrays
//! Uniform **object** arrays (every element is a JSON object, key ≠ `entries`) go to [`push_tables_sections`]
//! unless the array matches a **tabular** layout—then it becomes one titled block or indexed rows.
//! Tabular rules live in [`super::tabular`] (name/value objects, `[[str, v]]` pairs, string matrix rows).
//!
//! ## Column metadata
//! Compact per-column stats use [`column_metadata::push_column_metadata_sections`]. Parallel
//! `column_names` / `column_types` without compact `columns` uses
//! [`column_metadata::push_legacy_column_metadata_notice`].

use serde_json::{Map, Value};
use std::collections::HashSet;

use crate::config::ColumnStatsDisplay;
use crate::ui::UI_STRINGS;

use super::column_metadata;
use super::consts::SectionKeys;
use super::format;
use super::parse_ctx::KvParseCtx;
use super::schema;
use super::sections::{ContentsSection, KvSection, Section, SingleColumnListSection};
use super::tabular::{
    kv_rows_for_map_field, map_to_kv_rows_merging_tabular_lists, merged_tabular_rows_for_array,
};
use super::xlsx;

/// Well-known JSON field names in zahirscan metadata (and common display defaults).
pub struct WalkKeyVars;

impl WalkKeyVars {
    pub const ATTRIBUTES: &'static str = "attributes";
    pub const NAME: &'static str = "name";
    pub const VALUE: &'static str = "value";
    pub const PATH: &'static str = "path";
    pub const COLUMNS: &'static str = "columns";
    pub const METADATA: &'static str = "_metadata";
    /// Root Zahir field (stripped in [`map_without_display_file_type`]; category already shows type).
    pub const FILE_TYPE: &'static str = "file_type";
    pub const DEFAULT_TABLE_TITLE: &'static str = "Table";
    pub const DEFAULT_COLUMN_LABEL: &'static str = "column";
}

/// Per-walk limits for formatting JSON in table cells (see [`format::format_value`]).
pub type WalkCtx = KvParseCtx;

/// Clones `map_ref` and removes [`WalkKeyVars::FILE_TYPE`] so it is not shown again in KV tables.
fn map_without_display_file_type(map_ref: &Map<String, Value>) -> Map<String, Value> {
    let mut m = map_ref.clone();
    m.remove(WalkKeyVars::FILE_TYPE);
    m
}

/// Builds a contents table from a uniform object array: merged column keys, display headers, and row values.
///
/// Returns [`None`] if there are no objects. Key order starts from the first object; later objects may add keys.
fn object_array_to_contents_data(
    arr_ref: &[Value],
) -> Option<(Vec<String>, Vec<String>, Vec<Value>)> {
    let objs: Vec<&Map<String, Value>> = arr_ref.iter().filter_map(Value::as_object).collect();
    let first = objs.first()?;
    let mut column_keys: Vec<String> = first.keys().cloned().collect();
    let mut seen: HashSet<String> = column_keys.iter().cloned().collect();
    for obj in objs.iter().skip(1) {
        for k in obj.keys() {
            if seen.insert(k.clone()) {
                column_keys.push(k.clone());
            }
        }
    }
    let columns: Vec<String> = column_keys.iter().map(|k| format::format_key(k)).collect();
    let entries: Vec<Value> = arr_ref.iter().filter(|v| v.is_object()).cloned().collect();
    if entries.is_empty() {
        return None;
    }
    Some((column_keys, columns, entries))
}

/// Non-empty array whose elements are all JSON objects.
fn is_uniform_object_array(arr_ref: &[Value]) -> bool {
    !arr_ref.is_empty() && arr_ref.iter().all(Value::is_object)
}

/// True for uniform object arrays that should be expanded as record lists (not the root `entries` key).
#[inline]
fn array_is_record_table_list(key_ref: &str, arr_ref: &[Value]) -> bool {
    key_ref != SectionKeys::ENTRIES && is_uniform_object_array(arr_ref)
}

/// Pushes a contents table when `arr_ref` is suitable, then [`spill_entry_row_detail_sections`].
fn push_contents_from_entries(
    sections_mut_ref: &mut Vec<Section>,
    arr_ref: &[Value],
    ctx: WalkCtx,
) {
    if let Some((column_keys, columns, entries)) = object_array_to_contents_data(arr_ref) {
        sections_mut_ref.push(Section::Contents(ContentsSection {
            title: UI_STRINGS.tables.contents_title.to_string(),
            columns,
            column_keys,
            entries,
            sub_title: false,
        }));
    }
    spill_entry_row_detail_sections(sections_mut_ref, arr_ref, ctx);
}

/// Parses `map_ref` into a new `Vec` of sections (convenience wrapper around [`push_root_parts`]).
#[must_use]
pub fn root_parts_sections(
    map_ref: &Map<String, Value>,
    max_array_inline: usize,
    column_stats: ColumnStatsDisplay,
) -> Vec<Section> {
    let mut sections = Vec::new();
    push_root_parts(&mut sections, map_ref, max_array_inline, column_stats);
    sections
}

/// Walks the root map once and appends sections in a fixed order (see module docs).
pub fn push_root_parts(
    sections_mut_ref: &mut Vec<Section>,
    map_ref: &Map<String, Value>,
    max_array_inline: usize,
    column_stats: ColumnStatsDisplay,
) {
    let ctx = KvParseCtx::new(max_array_inline, column_stats);
    let map_owned = map_without_display_file_type(map_ref);
    push_root_parts_inner(sections_mut_ref, &map_owned, ctx);
}

/// Drains [`RootBuckets`] in phase order: flat, schema, xlsx, pivots, CSV column meta, record arrays, nested, entries.
fn push_root_parts_inner(
    sections_mut_ref: &mut Vec<Section>,
    map_ref: &Map<String, Value>,
    ctx: WalkCtx,
) {
    let mut buckets = RootBuckets::default();
    for (k, v) in map_ref {
        buckets.classify(k, v, ctx.max_array_inline);
    }

    if !buckets.flat.is_empty() {
        sections_mut_ref.push(Section::KeyValue(KvSection {
            title: None,
            rows: buckets.flat,
            sub_title: false,
        }));
    }
    if let Some(v) = buckets.schema_val {
        schema::push_schema_section(sections_mut_ref, &v);
    }
    if let Some((key, obj)) = buckets.sheet_stats {
        sections_mut_ref.push(xlsx::sheet_stats_to_section(&key, &obj));
    }
    if let Some(values) = buckets.common_pivots.filter(|pivots| !pivots.is_empty()) {
        sections_mut_ref.push(Section::SingleColumnList(SingleColumnListSection {
            title: format::format_key(SectionKeys::COMMON_PIVOTS),
            values,
        }));
    }
    if let Some(csv) = buckets.column_metadata {
        match csv {
            RootCsvMetadata::Compact { section_key, map } => {
                column_metadata::push_column_metadata_sections(
                    sections_mut_ref,
                    &section_key,
                    &map,
                    ctx.max_array_inline,
                    None,
                    ctx.typed_column_tables,
                );
            }
            RootCsvMetadata::Legacy { display_title } => {
                column_metadata::push_legacy_column_metadata_notice(
                    sections_mut_ref,
                    Some(display_title),
                    false,
                );
            }
        }
    }
    for (array_key, arr) in buckets.record_object_arrays {
        push_tables_sections(sections_mut_ref, arr, ctx, array_key, None);
    }
    for (key, m) in buckets.nested {
        process_nested_map(sections_mut_ref, &key, &m, &ctx, None);
    }
    if let Some(arr) = buckets.entries {
        push_contents_from_entries(sections_mut_ref, &arr, ctx);
    }
}

/// Classifier output for a single pass over the root zahir JSON object.
///
/// Each field is sorted into one bucket; [`push_root_parts_inner`] drains them in a fixed order.
/// Fields that need different section types cannot share one vector—hence multiple buckets.
///
/// - `flat` — Scalars, tabular-expanded arrays, and anything else that becomes rows in the first KV block.
/// - `schema`, `sheet_stats`, `common_pivots` — Handled by the schema / xlsx / list helpers.
/// - `column_metadata` — At most one of [`RootCsvMetadata::Compact`] or [`RootCsvMetadata::Legacy`].
/// - `record_object_arrays` — Borrows from the source map (uniform object lists for [`push_tables_sections`]).
/// - `nested` — Non-empty objects passed to [`process_nested_map`].
/// - `entries` — Optional `entries` array for contents + spill.
#[derive(Default)]
struct RootBuckets<'a> {
    flat: Vec<(String, String)>,
    nested: Vec<(String, Map<String, Value>)>,
    entries: Option<Vec<Value>>,
    schema_val: Option<Value>,
    sheet_stats: Option<(String, Map<String, Value>)>,
    common_pivots: Option<Vec<String>>,
    /// Filled from the `csv_metadata` key: compact `columns` layout or legacy parallel-array notice.
    column_metadata: Option<RootCsvMetadata>,
    record_object_arrays: Vec<(&'a str, &'a Vec<Value>)>,
}

/// Distinguishes current compact column metadata from old parallel-array-only blobs.
enum RootCsvMetadata {
    Compact {
        section_key: String,
        map: Map<String, Value>,
    },
    Legacy {
        display_title: String,
    },
}

impl<'a> RootBuckets<'a> {
    fn push_flat(&mut self, key: &str, val: &Value, max_array_inline: usize) {
        self.flat
            .extend(kv_rows_for_map_field(key, val, max_array_inline));
    }

    fn classify(&mut self, k: &'a str, v: &'a Value, max_array_inline: usize) {
        match k {
            SectionKeys::ENTRIES => self.entries = v.as_array().cloned(),
            SectionKeys::SCHEMA => self.schema_val = Some(v.clone()),
            SectionKeys::SHEET_STATS => self.classify_sheet_stats(k, v, max_array_inline),
            SectionKeys::COMMON_PIVOTS => self.classify_common_pivots(k, v, max_array_inline),
            SectionKeys::CSV_METADATA => self.classify_csv_metadata(k, v, max_array_inline),
            _ => self.classify_default(k, v, max_array_inline),
        }
    }

    fn classify_sheet_stats(&mut self, k: &str, v: &Value, max_array_inline: usize) {
        if let Some(obj) = v.as_object() {
            if xlsx::is_sheet_stats(obj) {
                self.sheet_stats = Some((k.to_string(), obj.clone()));
            } else {
                self.nested.push((k.to_string(), obj.clone()));
            }
            return;
        }
        self.push_flat(k, v, max_array_inline);
    }

    fn classify_common_pivots(&mut self, k: &str, v: &Value, max_array_inline: usize) {
        if let Some(arr) = v.as_array() {
            self.common_pivots = Some(
                arr.iter()
                    .map(|val| format::format_value(val, k, max_array_inline))
                    .collect(),
            );
            return;
        }
        self.push_flat(k, v, max_array_inline);
    }

    fn classify_csv_metadata(&mut self, k: &str, v: &Value, max_array_inline: usize) {
        if let Some(obj) = v.as_object() {
            if column_metadata::is_compact_column_metadata(obj) {
                self.column_metadata = Some(RootCsvMetadata::Compact {
                    section_key: k.to_string(),
                    map: obj.clone(),
                });
            } else if column_metadata::is_legacy_parallel_column_metadata(obj) {
                self.column_metadata = Some(RootCsvMetadata::Legacy {
                    display_title: format::format_key(k),
                });
            } else {
                self.nested.push((k.to_string(), obj.clone()));
            }
            return;
        }
        self.push_flat(k, v, max_array_inline);
    }

    fn classify_default(&mut self, k: &'a str, v: &'a Value, max_array_inline: usize) {
        match v {
            Value::Array(arr) if array_is_record_table_list(k, arr) => {
                self.record_object_arrays.push((k, arr));
            }
            Value::Object(m) if !m.is_empty() => self.nested.push((k.to_string(), m.clone())),
            _ => self.push_flat(k, v, max_array_inline),
        }
    }
}

/// Prefer `name`, then `path`, else `default_ref`.
#[inline]
fn object_name_or(obj_ref: &Map<String, Value>, default_ref: &str) -> String {
    obj_ref
        .get(WalkKeyVars::NAME)
        .and_then(Value::as_str)
        .map(String::from)
        .or_else(|| {
            obj_ref
                .get(WalkKeyVars::PATH)
                .and_then(Value::as_str)
                .map(String::from)
        })
        .unwrap_or_else(|| default_ref.to_string())
}

/// `parent · child` when `parent` is non-empty; otherwise `child_title` only.
fn join_with_parent_prefix(parent_title: Option<&str>, child_title: String) -> String {
    match parent_title {
        Some(p) if !p.is_empty() => format::join_dot([p, child_title.as_str()]),
        _ => child_title,
    }
}

/// Recurses into nested objects and record-list arrays on `object_ref`.
///
/// When `parent_kv_includes_merged_tabular` is true, skips arrays that [`super::tabular::merged_tabular_rows_for_array`]
/// would expand, because those rows are already in the parent KV table from [`super::tabular::map_to_kv_rows_merging_tabular_lists`].
fn expand_object_children_with_prefix(
    sections_mut_ref: &mut Vec<Section>,
    object_ref: &Map<String, Value>,
    ctx: WalkCtx,
    parent_title: Option<&str>,
    exclude_key: Option<&str>,
    parent_kv_includes_merged_tabular: bool,
) {
    for (k, val) in object_ref {
        if exclude_key == Some(k.as_str()) {
            continue;
        }
        match val {
            Value::Object(m) if !m.is_empty() => {
                process_nested_map(sections_mut_ref, k, m, &ctx, parent_title);
            }
            Value::Array(arr) if !arr.is_empty() && array_is_record_table_list(k.as_str(), arr) => {
                let is_repeat_tabular = parent_kv_includes_merged_tabular
                    && merged_tabular_rows_for_array(k, arr, ctx.max_array_inline)
                        .is_some_and(|r| !r.is_empty());
                if !is_repeat_tabular {
                    push_tables_sections(sections_mut_ref, arr, ctx, k.as_str(), parent_title);
                }
            }
            _ => {}
        }
    }
}

/// For each object in an `entries` array, walks nested fields with a per-row title prefix.
fn spill_entry_row_detail_sections(
    sections_mut_ref: &mut Vec<Section>,
    arr_ref: &[Value],
    ctx: WalkCtx,
) {
    for (i, v) in arr_ref.iter().enumerate() {
        let Some(obj) = v.as_object() else {
            continue;
        };
        let row_label = table_title_for_record(obj, Some(SectionKeys::ENTRIES), i);
        expand_object_children_with_prefix(
            sections_mut_ref,
            obj,
            ctx,
            Some(row_label.as_str()),
            None,
            false,
        );
    }
}

/// Title for one element of a record list: `name`, else `path`, else `Key · index`, else [`WalkKeyVars::DEFAULT_TABLE_TITLE`].
fn table_title_for_record(
    obj_ref: &Map<String, Value>,
    array_key: Option<&str>,
    index: usize,
) -> String {
    if let Some(n) = obj_ref.get(WalkKeyVars::NAME).and_then(Value::as_str) {
        return n.to_string();
    }
    if let Some(p) = obj_ref.get(WalkKeyVars::PATH).and_then(Value::as_str) {
        return p.to_string();
    }
    if let Some(k) = array_key {
        return format::join_dot([format::format_key(k), index.to_string()]);
    }
    WalkKeyVars::DEFAULT_TABLE_TITLE.to_string()
}

/// One sub-section per object in a uniform object array, or one merged table for tabular arrays.
fn push_tables_sections(
    sections_mut_ref: &mut Vec<Section>,
    arr_ref: &[Value],
    walk_ctx: WalkCtx,
    array_key: &str,
    title_row_prefix: Option<&str>,
) {
    if let Some(flat) = merged_tabular_rows_for_array(array_key, arr_ref, walk_ctx.max_array_inline)
        && !flat.is_empty()
    {
        let table_name = join_with_parent_prefix(title_row_prefix, format::format_key(array_key));
        sections_mut_ref.push(Section::KeyValue(KvSection {
            title: Some(table_name),
            rows: flat,
            sub_title: title_row_prefix.is_some_and(|p| !p.is_empty()),
        }));
        return;
    }
    for (i, v) in arr_ref.iter().enumerate() {
        let Some(v) = v.as_object() else {
            continue;
        };
        let table_name = join_with_parent_prefix(
            title_row_prefix,
            table_title_for_record(v, Some(array_key), i),
        );
        if column_metadata::is_compact_column_metadata(v) {
            let title_override = title_row_prefix
                .is_some_and(|p| !p.is_empty())
                .then_some(table_name.as_str());
            column_metadata::push_column_metadata_sections(
                sections_mut_ref,
                table_name.as_str(),
                v,
                walk_ctx.max_array_inline,
                title_override,
                walk_ctx.typed_column_tables,
            );
            continue;
        }
        if column_metadata::is_legacy_parallel_column_metadata(v) {
            column_metadata::push_legacy_column_metadata_notice(
                sections_mut_ref,
                Some(table_name.clone()),
                true,
            );
            continue;
        }
        let rows = map_to_kv_rows_merging_tabular_lists(
            v,
            Some(WalkKeyVars::COLUMNS),
            walk_ctx.max_array_inline,
        );
        if !rows.is_empty() {
            sections_mut_ref.push(Section::KeyValue(KvSection {
                title: Some(table_name.clone()),
                rows,
                sub_title: false,
            }));
        }
        if let Some(col_arr) = v.get(WalkKeyVars::COLUMNS).and_then(Value::as_array) {
            if column_metadata::is_compact_column_stats_array(col_arr) {
                sections_mut_ref.extend(column_metadata::typed_sections_from_compact_columns(
                    col_arr,
                    Some(table_name.as_str()),
                    walk_ctx.typed_column_tables,
                ));
            } else if let Some((column_keys, columns, entries)) =
                object_array_to_contents_data(col_arr)
            {
                sections_mut_ref.push(Section::Contents(ContentsSection {
                    title: format::join_dot([&table_name, UI_STRINGS.tables.columns_title]),
                    columns,
                    column_keys,
                    entries: entries.clone(),
                    sub_title: true,
                }));
                push_column_stats_sections(
                    sections_mut_ref,
                    &table_name,
                    &entries,
                    walk_ctx.max_array_inline,
                    walk_ctx.typed_column_tables,
                );
            }
        }
        // Spill nested fields on each record; `columns` is handled in the block above.
        expand_object_children_with_prefix(
            sections_mut_ref,
            v,
            walk_ctx,
            Some(table_name.as_str()),
            Some(WalkKeyVars::COLUMNS),
            true,
        );
    }
}

/// Per-column extra stats objects under a sheet-style `columns` list.
fn push_column_stats_sections(
    sections_mut_ref: &mut Vec<Section>,
    table_name_ref: &str,
    column_objs_ref: &[Value],
    max_array_inline: usize,
    mode: ColumnStatsDisplay,
) {
    if !mode.shows_tables() {
        return;
    }
    for col in column_objs_ref.iter().filter_map(Value::as_object) {
        let col_name = object_name_or(col, WalkKeyVars::DEFAULT_COLUMN_LABEL);
        for (stats_key, stats_val) in col {
            if stats_key == WalkKeyVars::NAME {
                continue;
            }
            if let Some(stats_obj) = stats_val.as_object() {
                let rows = column_metadata::nested_stats_kv_rows(
                    stats_key,
                    stats_obj,
                    max_array_inline,
                    mode,
                );
                if !rows.is_empty() {
                    let stats_label = format::format_key(stats_key);
                    sections_mut_ref.push(Section::KeyValue(KvSection {
                        title: Some(format::join_dot([
                            table_name_ref,
                            col_name.as_str(),
                            stats_label.as_str(),
                        ])),
                        rows,
                        sub_title: true,
                    }));
                }
            }
        }
    }
}

/// Walks a nested JSON object: special keys first, then a flat KV block, then child maps and arrays.
///
/// `section_key_ref` and `parent_title` form the section title for this level (`parent ·` + formatted key when nested).
/// Deep child objects (e.g. a stats map) get their own KV section instead of a `{…}` placeholder.
pub fn process_nested_map(
    sections_mut_ref: &mut Vec<Section>,
    section_key_ref: &str,
    map_ref: &Map<String, Value>,
    ctx_ref: &WalkCtx,
    parent_title: Option<&str>,
) {
    let own_title = join_with_parent_prefix(parent_title, format::format_key(section_key_ref));
    if column_metadata::is_compact_column_metadata(map_ref) {
        column_metadata::push_column_metadata_sections(
            sections_mut_ref,
            section_key_ref,
            map_ref,
            ctx_ref.max_array_inline,
            Some(own_title.as_str()),
            ctx_ref.typed_column_tables,
        );
        return;
    }
    if column_metadata::is_legacy_parallel_column_metadata(map_ref) {
        column_metadata::push_legacy_column_metadata_notice(
            sections_mut_ref,
            Some(own_title),
            false,
        );
        return;
    }

    let mut flat = Vec::new();
    let mut entries = None;
    let mut schema_val = None;
    let mut common_pivots: Option<Vec<String>> = None;
    let mut record_object_arrays: Vec<(&str, &Vec<Value>)> = Vec::new();
    let mut child_objects: Vec<(String, Map<String, Value>)> = Vec::new();

    for (k, v) in map_ref {
        match k.as_str() {
            SectionKeys::ENTRIES => entries = v.as_array().cloned(),
            SectionKeys::SCHEMA => schema_val = Some(v.clone()),
            SectionKeys::COMMON_PIVOTS => {
                if let Some(arr) = v.as_array() {
                    let mi = ctx_ref.max_array_inline;
                    common_pivots = Some(
                        arr.iter()
                            .map(|val| format::format_value(val, k, mi))
                            .collect(),
                    );
                } else {
                    flat.extend(kv_rows_for_map_field(k, v, ctx_ref.max_array_inline));
                }
            }
            _ => match v {
                Value::Object(m) if !m.is_empty() => {
                    child_objects.push((k.clone(), m.clone()));
                }
                Value::Array(arr) if array_is_record_table_list(k.as_str(), arr) => {
                    record_object_arrays.push((k.as_str(), arr));
                }
                _ => flat.extend(kv_rows_for_map_field(k, v, ctx_ref.max_array_inline)),
            },
        }
    }

    if !flat.is_empty() {
        sections_mut_ref.push(Section::KeyValue(KvSection {
            title: Some(own_title.clone()),
            rows: flat,
            sub_title: false,
        }));
    }
    for (child_key, child_map) in child_objects {
        process_nested_map(
            sections_mut_ref,
            &child_key,
            &child_map,
            ctx_ref,
            Some(own_title.as_str()),
        );
    }
    if let Some(v) = schema_val {
        schema::push_schema_section(sections_mut_ref, &v);
    }
    if let Some(values) = common_pivots.filter(|pivots| !pivots.is_empty()) {
        sections_mut_ref.push(Section::SingleColumnList(SingleColumnListSection {
            title: format::format_key(SectionKeys::COMMON_PIVOTS),
            values,
        }));
    }
    if let Some(arr) = entries {
        push_contents_from_entries(sections_mut_ref, &arr, *ctx_ref);
    }
    for (ak, arr) in record_object_arrays {
        push_tables_sections(sections_mut_ref, arr, *ctx_ref, ak, None);
    }
}
