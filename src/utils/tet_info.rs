//! `.tet` viewer body: same text as `tet info --all -n 0` (full catalog sections, all chunk rows).

use std::path::Path;

use tetration::catalog::read_tet_summary_v1;
use tetration::layout::mmap_file_read;
use tetration::query::{InfoMetadataDisplay, InfoViewSections, format_info_text};

/// `true` when `path` should use the tetration catalog viewer instead of raw bytes.
#[inline]
#[must_use]
pub fn path_is_tet_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("tet"))
}

/// Human-readable catalog report for a `.tet` file (mmap + summary parse).
#[must_use]
pub fn tet_info_text_for_viewer(path: &Path) -> String {
    match tet_info_text_for_viewer_result(path) {
        Ok(s) => s,
        Err(e) => format!("(tet info failed: {e})"),
    }
}

fn tet_info_text_for_viewer_result(path: &Path) -> Result<String, String> {
    let mmap = mmap_file_read(path).map_err(|e| e.to_string())?;
    let file_len = u64::try_from(mmap.len())
        .map_err(|_| format!("file size {} exceeds u64::MAX", mmap.len()))?;
    let summary = read_tet_summary_v1(&mmap).map_err(|e| e.to_string())?;
    Ok(format_info_text(
        Some(path),
        file_len,
        &summary,
        None,
        InfoViewSections::all(),
        0,
        InfoMetadataDisplay::WhenPresent,
    ))
}
