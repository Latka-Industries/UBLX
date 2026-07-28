//! Key/value and value display formatting for metadata and contents tables.

#[cfg(feature = "tui")]
use ratatui::style::Style;

#[cfg(feature = "tui")]
use crate::themes::DEFAULT_COLORS;
use crate::ui::UI_STRINGS;
use crate::utils::{Epsilon, format_bytes};

/// Words that are displayed in full caps (e.g. "svo" -> "SVO", "pdf" -> "PDF").
const ALL_CAPS: &[&str] = &[
    "svo", "pdf", "tiff", "jpeg", "png", "rgb", "yuv", "aac", "mp3", "h264", "cbr", "lf", "und",
    "eng",
];

const FLOAT_PRECISION: usize = 4;

/// When the UI width is unknown (e.g. tests), use the historical cap of 3 short primitives inline.
pub const DEFAULT_MAX_ARRAY_INLINE: usize = 3;

/// Max characters of **value** column to assume when approximating `max_array_inline` from the full table width (key col + padding).
const VALUE_COL_ASSUMED_OVERHEAD: u16 = 38;

#[inline]
#[must_use]
pub fn is_byte_key(key: &str) -> bool {
    key.to_lowercase().contains("size")
        || key.to_lowercase().contains("compressed")
        || key.to_lowercase().contains("uncompressed")
        || key.to_lowercase().contains("byte")
}

#[must_use]
pub fn is_bool(value: &serde_json::Value) -> bool {
    matches!(value, serde_json::Value::Bool(_))
}

/// Optional style for a value cell (e.g. TRUE = green, FALSE = red). Returns `None` for non-bool values.
#[cfg(feature = "tui")]
#[must_use]
pub fn value_cell_style(formatted_value: &str) -> Option<Style> {
    match formatted_value {
        "TRUE" => Some(Style::default().fg(DEFAULT_COLORS.green)),
        "FALSE" => Some(Style::default().fg(DEFAULT_COLORS.red)),
        _ => None,
    }
}

/// Join parts with middle dot: 2 parts → "a · b", 3 parts → "a · b · c".
pub fn join_dot(parts: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    parts
        .into_iter()
        .map(|p| p.as_ref().to_string())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Schema tree: prefix + label (e.g. tree branch + node name).
#[must_use]
pub fn prefixed_label(prefix: &str, label: &str) -> String {
    format!("{prefix}{label}")
}

/// Schema tree: prefix + label + ": " + value string (leaf line).
#[must_use]
pub fn prefixed_label_with_value(prefix: &str, label: &str, value_str: &str) -> String {
    format!("{prefix}{label}: {value_str}")
}

#[must_use]
pub fn format_key(key: &str) -> String {
    let words: Vec<String> = key
        .split('_')
        .map(|w| {
            let lower = w.to_lowercase();
            if ALL_CAPS.iter().any(|&c| c == lower) {
                w.to_uppercase()
            } else {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(c).collect(),
                }
            }
        })
        .collect();
    words.join(" ")
}

/// Heuristic: how many primitive array items to show comma-separated before falling back to full JSON text.
/// Wider value columns allow more; narrow panes stay near [`DEFAULT_MAX_ARRAY_INLINE`].
#[must_use]
pub fn max_array_inline_for_value_width(value_width: u16) -> usize {
    let w = (value_width.max(1) as usize).saturating_sub(2);
    // ~5 chars per small int plus ", " between elements.
    let n = w / 5;
    n.clamp(DEFAULT_MAX_ARRAY_INLINE, 128)
}

/// Approximate value-column width in characters from a rendered table’s inner width.
#[inline]
#[must_use]
pub fn value_width_from_table_width(table_inner_width: u16) -> u16 {
    table_inner_width
        .saturating_sub(VALUE_COL_ASSUMED_OVERHEAD)
        .max(6)
}

#[must_use]
pub fn value_to_string(v: &serde_json::Value, max_array_inline: usize) -> String {
    let max_inline = max_array_inline.max(1);
    match v {
        serde_json::Value::Null => "—".to_string(),
        serde_json::Value::Bool(b) => b.to_string().to_uppercase(),
        serde_json::Value::Number(n) => n
            .as_f64()
            .filter(|f| f.fract().abs() >= Epsilon::FORMAT)
            .map_or_else(|| n.to_string(), |f| format!("{f:.FLOAT_PRECISION$}")),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else if arr.len() <= max_inline && arr.iter().all(|x| x.is_string() || x.is_number())
            {
                arr.iter()
                    .map(|e| value_to_string(e, max_inline))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                // Full array as JSON (not `[n items]`) so size in the cell is easy to eyeball.
                serde_json::to_string(v)
                    .unwrap_or_else(|_| format!("[{} {}]", arr.len(), UI_STRINGS.misc.json_items))
            }
        }
        serde_json::Value::Object(_) => "{…}".to_string(),
    }
}

/// Non-negative JSON floats for byte-like keys: clamp to `u64`, then truncate toward zero.
fn json_f64_to_u64_for_bytes(f: f64) -> u64 {
    if f <= 0.0 || !f.is_finite() {
        return 0;
    }
    if f >= u64::MAX as f64 {
        return u64::MAX;
    }
    f as u64
}

/// Format value for display: byte format when key contains "size", "compressed", or "uncompressed" (and value is numeric); "%" when key contains "percent" (case-insensitive). `max_array_inline` controls primitive array joining vs full JSON text for other arrays.
#[must_use]
pub fn format_value(v: &serde_json::Value, key: &str, max_array_inline: usize) -> String {
    let key_lower = key.to_lowercase();
    if is_byte_key(&key_lower) {
        if let Some(n) = v.as_u64() {
            return format_bytes(n);
        }
        if let Some(n) = v.as_i64().filter(|&x| x >= 0) {
            // Non-negative `i64` → `u64` without `as` sign-loss ambiguity.
            return format_bytes(n.cast_unsigned());
        }
        if let Some(f) = v.as_f64().filter(|&x| x >= 0.0 && x.is_finite()) {
            return format_bytes(json_f64_to_u64_for_bytes(f));
        }
    }
    let s = value_to_string(v, max_array_inline);
    if key_lower.contains("percent") {
        format!("{s}%")
    } else {
        s
    }
}
