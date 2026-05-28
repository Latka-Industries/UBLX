//! Tabular JSON shapes that merge into key/value rows (name-value objects, `[[str,v]]` pairs, string rows).

use serde_json::Value;

use super::WalkKeyVars;
use super::format;

const VALIDATED_PAIR: &str = "validated";

/// `Some(rows)` when `arr` is a non-empty list of objects, each with string [`WalkKeyVars::NAME`] and any [`WalkKeyVars::VALUE`].
pub(super) fn try_flatten_name_value_list(
    arr_ref: &[Value],
    max_array_inline: usize,
) -> Option<Vec<(String, String)>> {
    if arr_ref.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(arr_ref.len());
    for v in arr_ref {
        let obj = v.as_object()?;
        let name = obj.get(WalkKeyVars::NAME)?.as_str()?;
        let value = obj.get(WalkKeyVars::VALUE)?;
        out.push((
            name.to_string(),
            format::format_value(value, name, max_array_inline),
        ));
    }
    Some(out)
}

/// Value cell for one `[name, value]` pair row (e.g. `[y, 4096]`).
fn pair_row_bracket_display(pair: &[Value], max_array_inline: usize) -> String {
    let name = pair[0].as_str().unwrap_or("");
    let val = format::value_to_string(&pair[1], max_array_inline);
    format!("[{name}, {val}]")
}

/// `Some(rows)` when `arr` is a non-empty list of length-2 JSON arrays, each `[string, any]`.
/// Row keys: `"{pretty_json_key} {index}"` (e.g. `Dimensions Sample 0`).
pub(super) fn indexed_pair_array_rows(
    json_key: &str,
    arr_ref: &[Value],
    max_array_inline: usize,
) -> Option<Vec<(String, String)>> {
    if arr_ref.is_empty() {
        return None;
    }
    for v in arr_ref {
        let pair = v.as_array()?;
        if pair.len() != 2 || pair[0].as_str().is_none() {
            return None;
        }
    }
    let prefix = format::format_key(json_key);
    Some(
        arr_ref
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let pair = v.as_array().expect(VALIDATED_PAIR);
                (
                    format!("{prefix} {i}"),
                    pair_row_bracket_display(pair, max_array_inline),
                )
            })
            .collect(),
    )
}

/// `Some(rows)` for `[[ "a", "b" ], [ "c" ], …]` (all strings per inner array); tried before
/// `indexed_pair_array_rows` so `["y", 4096]` is still a dimension pair.
pub(super) fn indexed_string_matrix_rows(
    json_key: &str,
    arr_ref: &[Value],
) -> Option<Vec<(String, String)>> {
    if arr_ref.is_empty() {
        return None;
    }
    for v in arr_ref {
        let inner = v.as_array()?;
        if inner.is_empty() {
            return None;
        }
        if !inner.iter().all(Value::is_string) {
            return None;
        }
    }
    let prefix = format::format_key(json_key);
    Some(
        arr_ref
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let inner = v.as_array().expect(VALIDATED_PAIR);
                let parts: Vec<&str> = inner.iter().filter_map(Value::as_str).collect();
                let joined = format::join_dot(parts);
                (format!("{prefix} {i}"), joined)
            })
            .collect(),
    )
}

/// Name/value object lists, else string-matrix rows, else indexed `[str, any]` pair rows.
pub(super) fn merged_tabular_rows_for_array(
    json_key: &str,
    arr_ref: &[Value],
    max_array_inline: usize,
) -> Option<Vec<(String, String)>> {
    try_flatten_name_value_list(arr_ref, max_array_inline)
        .or_else(|| indexed_string_matrix_rows(json_key, arr_ref))
        .or_else(|| indexed_pair_array_rows(json_key, arr_ref, max_array_inline))
}

/// One map field: expand tabular arrays into multiple rows, else a single formatted row.
pub(super) fn kv_rows_for_map_field(
    json_key: &str,
    val: &Value,
    max_array_inline: usize,
) -> Vec<(String, String)> {
    if let Some(arr) = val.as_array()
        && let Some(rows) = merged_tabular_rows_for_array(json_key, arr, max_array_inline)
    {
        return rows;
    }
    vec![(
        format::format_key(json_key),
        format::format_value(val, json_key, max_array_inline),
    )]
}

/// Flatten a map to key/value rows; array fields with tabular shapes merge as multiple rows.
pub(super) fn map_to_kv_rows_merging_tabular_lists(
    map_ref: &serde_json::Map<String, Value>,
    exclude_key: Option<&str>,
    max_array_inline: usize,
) -> Vec<(String, String)> {
    map_ref
        .iter()
        .filter(|(k, _)| exclude_key != Some(k.as_str()))
        .flat_map(|(k, val)| kv_rows_for_map_field(k, val, max_array_inline))
        .collect()
}
