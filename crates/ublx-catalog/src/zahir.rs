//! Pure `zahirscan` helpers the snapshot tables need: file-type labels for `category` and
//! `zahir_json` serialization. The `extract_zahir` runners stay in `ublx` (`integrations::zahir_ops`).

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

pub type ZahirResult = zahirscan::ZahirScanResult;
pub type ZahirOutput = zahirscan::Output;
pub type ZahirFT = zahirscan::FileType;

/// Parse a DB `category` string into [`ZahirFT`] when it matches `FileType::as_metadata_name`.
///
/// Delegates to `FileType::from_metadata_name` (zahirscan); full round-trip tests live there.
#[must_use]
pub fn file_type_from_metadata_name(s: &str) -> Option<ZahirFT> {
    ZahirFT::from_metadata_name(s)
}

fn metadata_name_from_detect_key(key: &str) -> Option<String> {
    let ft = zahirscan::utils::filetypes::detect_file_type(key);
    (ft != ZahirFT::Unknown).then(|| ft.as_metadata_name().to_string())
}

/// Metadata name string for [`ZahirFT`] from path/extension only (ZahirScan's `detect_file_type`), without a full extract.
///
/// **Caveat:** zahirscan's linguist fallback uses `Path::new(path_str).exists()`, which is relative to **process cwd**.
/// For indexed trees when cwd ≠ project root (e.g. `ublx /path/to/repo`), use [`zahir_metadata_name_from_indexed_file`].
#[must_use]
pub fn zahir_metadata_name_from_path_hint(path_str: &str) -> Option<String> {
    metadata_name_from_detect_key(path_str)
}

/// Like [`zahir_metadata_name_from_path_hint`], but uses `full_path` (e.g. `dir_to_ublx.join(rel)`) for
/// `detect_file_type` when that path exists so `.py` / `.rs` / linguist work regardless of cwd.
#[must_use]
pub fn zahir_metadata_name_from_indexed_file(full_path: &Path, path_str: &str) -> Option<String> {
    let key = if full_path.exists() {
        full_path.to_string_lossy().into_owned()
    } else {
        path_str.to_string()
    };
    metadata_name_from_detect_key(&key)
}

/// Zahir output by path from a zahir result. Keys are path strings.
/// If `dir_to_ublx_abs` is `Some`, keys are relative to it (so they line up with nefaxer); otherwise keys are absolute (source as-is).
#[must_use]
pub fn get_zahir_output_by_path<'a>(
    zahir_result: &'a ZahirResult,
    dir_to_ublx_abs: Option<&Path>,
) -> HashMap<String, &'a ZahirOutput> {
    zahir_result
        .outputs
        .iter()
        .filter_map(|o| {
            let s = o.source.as_ref()?;
            let key = match dir_to_ublx_abs {
                Some(r) => Path::new(s)
                    .strip_prefix(r)
                    .ok()?
                    .to_string_lossy()
                    .into_owned(),
                None => s.clone(),
            };
            Some((key, o))
        })
        .collect()
}

/// Convert a zahir output to a JSON string (no path-based `file_type` fill-in).
#[must_use]
pub fn zahir_output_to_json(output: Option<&ZahirOutput>) -> String {
    output
        .and_then(|o| serde_json::to_string(o).ok())
        .unwrap_or_default()
}

fn zahir_json_needs_path_file_type(v: &Value) -> bool {
    match v.get("file_type") {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        _ => false,
    }
}

/// Merge path-based `file_type` when it is missing or empty (uses indexed path for cwd-safe detection).
fn inject_path_detected_file_type(v: &mut Value, full_path: &Path, path_str: &str) {
    if !zahir_json_needs_path_file_type(v) {
        return;
    }
    let Some(name) = zahir_metadata_name_from_indexed_file(full_path, path_str) else {
        return;
    };
    if let Some(obj) = v.as_object_mut() {
        obj.insert("file_type".to_string(), Value::String(name));
    }
}

/// Serialize [`ZahirOutput`] for DB storage. When `file_type` is absent or empty, sets it from
/// zahirscan's `detect_file_type` using `full_path` when it exists (same labels as full extract).
#[must_use]
pub fn zahir_output_to_json_for_path(
    output: Option<&ZahirOutput>,
    full_path: &Path,
    path_str: &str,
) -> String {
    let Some(o) = output else {
        return String::new();
    };
    let Ok(mut v) = serde_json::to_value(o) else {
        return zahir_output_to_json(Some(o));
    };
    inject_path_detected_file_type(&mut v, full_path, path_str);
    serde_json::to_string(&v).unwrap_or_default()
}
