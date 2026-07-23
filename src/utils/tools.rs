use colored::Colorize;
use log::{Level, debug, error};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::UBLX_NAMES;
use crate::integrations::ZahirFT;
use crate::utils::{exit_error, path_is_tet_file, tet_info_text_for_viewer};

/// Binary prefixes (1024-based), shared with [`format_bytes`].
pub struct ByteUnits;
impl ByteUnits {
    pub const KIB: u64 = 1024;
    pub const MIB: u64 = Self::KIB * 1024;
    pub const GIB: u64 = Self::MIB * 1024;
}

/// Viewer read policy values used by [`file_content_for_viewer`].
pub struct ViewerReadPolicy;
impl ViewerReadPolicy {
    /// Half a megabyte (512 KiB): max single-file text load in the viewer and minimum file size
    /// for off-thread image decode — keep these policies in sync unless intentionally diverging.
    pub const HALF_MIB_BYTES: u64 = ByteUnits::MIB / 2;
    /// [`HALF_MIB_BYTES`] as [`usize`] for allocation caps (`HALF_MIB_BYTES` always fits in `usize`).
    pub const HALF_MIB_BYTES_USIZE: usize = Self::HALF_MIB_BYTES as usize;
    /// Per-chunk size for large-text head + tail previews: two chunks sum to [`HALF_MIB_BYTES`].
    pub const TEXT_HEAD_TAIL_CHUNK_BYTES: u64 = Self::HALF_MIB_BYTES / 2;
    /// Delimited viewer preview keeps many rows while bounding disk read work.
    pub const CSV_VIEWER_MAX_BYTES: u64 = 4 * ByteUnits::MIB;
    /// Hard cap for preview rows loaded from large delimited files.
    pub const CSV_VIEWER_MAX_LINES: usize = 2_000;
    /// Synthetic trailer line for CSV preview metadata (parsed/stripped in `csv_handler`).
    pub const CSV_TOTAL_ROWS_META_PREFIX: &'static str = "__UBLX_CSV_TOTAL_ROWS__=";
    /// Bytes read by binary sniffing (`is_likely_binary`) to detect NUL / invalid UTF-8.
    pub const BINARY_CHECK_CHUNK: usize = 8192;
}

// Back-compat aliases used across the file/module.
pub const KIB: u64 = ByteUnits::KIB;
pub const MIB: u64 = ByteUnits::MIB;
pub const GIB: u64 = ByteUnits::GIB;

/// [`std::fs::Metadata::len`] is `u64`; saturates at `usize::MAX` on 32-bit. Safe for `.min(small_cap)`:
/// the cap (e.g. [`HALF_MIB_BYTES_USIZE`]) still bounds allocation.
#[inline]
#[must_use]
pub fn u64_to_usize_saturating(len: u64) -> usize {
    usize::try_from(len).unwrap_or(usize::MAX)
}

/// Quick binary heuristic: read the first chunk; binary if NUL present or invalid UTF-8.
#[must_use]
pub fn is_likely_binary(path: &Path) -> bool {
    let Ok(mut f) = fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; ViewerReadPolicy::BINARY_CHECK_CHUNK];
    let n = f.read(&mut buf).unwrap_or(0);
    let buf = &buf[..n];
    buf.contains(&0) || std::str::from_utf8(buf).is_err()
}

/// Label for a binary file: `"EXT file"` if the path has an extension (e.g. `"PNG file"`), else `"binary file"`.
#[must_use]
pub fn binary_file_label(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).map_or_else(
        || "binary file".to_string(),
        |ext| format!("{} file", ext.to_uppercase()),
    )
}

/// Read first + last chunks for large text files; same total byte budget as [`HALF_MIB_BYTES`].
fn read_text_head_tail(path: &Path, total: u64) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let chunk = ViewerReadPolicy::TEXT_HEAD_TAIL_CHUNK_BYTES;
    let mut head = Vec::new();
    let n_head = Read::by_ref(&mut f)
        .take(chunk)
        .read_to_end(&mut head)
        .ok()?;

    let tail_len = chunk.min(total);
    let start = total.saturating_sub(tail_len);
    f.seek(std::io::SeekFrom::Start(start)).ok()?;
    let mut tail = Vec::new();
    let n_tail = Read::by_ref(&mut f)
        .take(chunk)
        .read_to_end(&mut tail)
        .ok()?;

    let head_s = String::from_utf8_lossy(&head[..n_head]);
    let tail_s = String::from_utf8_lossy(&tail[..n_tail]);
    let n_head_u = n_head as u64;
    let n_tail_u = n_tail as u64;
    let omitted = total.saturating_sub(n_head_u.saturating_add(n_tail_u));

    Some(format!(
        "{head_s}\n\n… ─── {omitted} bytes omitted (middle of {total} byte file) ─── …\n\n{tail_s}"
    ))
}

/// Read an initial line window for delimited files without injecting truncation markers into data.
///
/// This preserves parseability for the CSV renderer while allowing many more visible rows than
/// a pure byte cap on very wide rows.
fn read_delimited_preview_lines(path: &Path) -> Option<String> {
    let f = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(f);
    let mut out = String::new();
    let mut line = String::new();
    let mut total_bytes = 0u64;
    let mut line_count = 0usize;
    let mut reached_eof = false;

    while line_count < ViewerReadPolicy::CSV_VIEWER_MAX_LINES
        && total_bytes < ViewerReadPolicy::CSV_VIEWER_MAX_BYTES
    {
        line.clear();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            reached_eof = true;
            break;
        }
        total_bytes = total_bytes.saturating_add(n as u64);
        out.push_str(&line);
        line_count += 1;
    }
    if !reached_eof {
        let mut total_rows = line_count;
        loop {
            line.clear();
            let n = reader.read_line(&mut line).ok()?;
            if n == 0 {
                break;
            }
            total_rows += 1;
        }
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(ViewerReadPolicy::CSV_TOTAL_ROWS_META_PREFIX);
        out.push_str(&total_rows.to_string());
        out.push('\n');
    }
    Some(out)
}

/// Read a UTF-8 byte window from `path` for serve windowing (`offset` / `limit` on `/content`).
///
/// Caps `limit` at [`ViewerReadPolicy::HALF_MIB_BYTES`]. Invalid UTF-8 is lossy-decoded.
#[must_use]
pub fn read_text_byte_window(path: &Path, offset: u64, limit: u64) -> Option<TextByteWindow> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let total = meta.len();
    let offset = offset.min(total);
    let limit = limit
        .min(ViewerReadPolicy::HALF_MIB_BYTES)
        .min(total.saturating_sub(offset));
    if limit == 0 {
        return Some(TextByteWindow {
            text: String::new(),
            offset,
            byte_len: 0,
            total,
        });
    }
    let mut f = fs::File::open(path).ok()?;
    f.seek(std::io::SeekFrom::Start(offset)).ok()?;
    let mut buf = Vec::with_capacity(u64_to_usize_saturating(limit));
    let n = Read::by_ref(&mut f)
        .take(limit)
        .read_to_end(&mut buf)
        .ok()?;
    Some(TextByteWindow {
        text: String::from_utf8_lossy(&buf[..n]).into_owned(),
        offset,
        byte_len: n as u64,
        total,
    })
}

/// One byte window from [`read_text_byte_window`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextByteWindow {
    pub text: String,
    pub offset: u64,
    pub byte_len: u64,
    pub total: u64,
}

impl TextByteWindow {
    #[must_use]
    pub fn next_offset(&self) -> Option<u64> {
        let next = self.offset.saturating_add(self.byte_len);
        (next < self.total).then_some(next)
    }

    #[must_use]
    pub fn prev_offset(&self, window: u64) -> u64 {
        self.offset.saturating_sub(window.max(1))
    }
}

/// Read text for a viewer pane: not found, empty for image/PDF (raster preview elsewhere), binary short label, or capped UTF-8 with truncation notice.
///
/// Cap is [`HALF_MIB_BYTES_USIZE`]. Files larger than that read **head + tail** (half cap each) with an explicit omitted-bytes marker instead of loading the full buffer. When `zahir_type` is [`FileType::Image`], [`FileType::Pdf`], or [`FileType::Video`], returns empty string for a normal file so the render layer loads the preview.
#[must_use]
pub fn file_content_for_viewer(path: &Path, zahir_type: Option<ZahirFT>) -> Option<String> {
    let Ok(meta) = fs::metadata(path) else {
        return Some("(file not found)".to_string());
    };
    if meta.is_dir() && zahir_type == Some(ZahirFT::Zarr) {
        return Some(String::new());
    }
    if meta.is_file()
        && matches!(
            zahir_type,
            Some(ZahirFT::Image | ZahirFT::Pdf | ZahirFT::Video)
        )
    {
        return Some(String::new());
    }
    if meta.is_file() && (zahir_type == Some(ZahirFT::Tetration) || path_is_tet_file(path)) {
        return Some(tet_info_text_for_viewer(path));
    }
    if meta.is_file() && is_likely_binary(path) {
        return Some(binary_file_label(path));
    }
    let len = meta.len();
    if zahir_type == Some(ZahirFT::Csv) {
        return read_delimited_preview_lines(path);
    }
    if len > ViewerReadPolicy::HALF_MIB_BYTES {
        return read_text_head_tail(path, len);
    }

    let f = fs::File::open(path).ok()?;
    let cap = ViewerReadPolicy::HALF_MIB_BYTES_USIZE.min(u64_to_usize_saturating(len));
    let mut buf = Vec::with_capacity(cap);
    let n = f.take(len).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf[..n]).into_owned())
}

/// Expand a leading `~/` using `HOME` so `cargo run -- ~/src/proj` works (the shell often does not expand `~` in argv).
#[must_use]
pub fn expand_home_dir_arg(path: &Path) -> PathBuf {
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    let Some(rest) = s.strip_prefix("~/") else {
        return path.to_path_buf();
    };
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    #[cfg(windows)]
    {
        if let Ok(user) = std::env::var("USERPROFILE") {
            return PathBuf::from(user).join(rest);
        }
    }
    path.to_path_buf()
}

/// Validate that a path is a directory and return the canonicalized path.
/// Symlinks are resolved (e.g. `~/Dropbox` → `~/Library/CloudStorage/...` on macOS).
#[must_use]
pub fn validate_dir(path: &std::path::Path) -> PathBuf {
    let path = expand_home_dir_arg(path);
    if path.exists() && !path.is_dir() {
        error!("'{}' is not a directory", path.display());
        exit_error();
    }
    if !path.exists() {
        error!("'{}' no such file or directory", path.display());
        exit_error();
    }
    path.canonicalize().unwrap_or_else(|e| {
        error!("cannot canonicalize '{}': {}", path.display(), e);
        exit_error();
    })
}

/// Like [`validate_dir`] but returns `Err` instead of exiting (e.g. first-run path input).
///
/// # Errors
///
/// Returns `Err` with a message if the path does not exist, is not a directory, or cannot be canonicalized.
pub fn try_validate_dir(path: &Path) -> Result<PathBuf, String> {
    let path = expand_home_dir_arg(path);
    if !path.exists() {
        return Err(format!("no such file or directory: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("not a directory: {}", path.display()));
    }
    path.canonicalize()
        .map_err(|e| format!("cannot canonicalize '{}': {e}", path.display()))
}

#[must_use]
pub fn canonicalize_dir_to_ublx(dir_to_ublx: &Path) -> PathBuf {
    dir_to_ublx
        .canonicalize()
        .unwrap_or_else(|_| dir_to_ublx.to_path_buf())
}

/// Color the level of the log message.
fn level_colored(level: Level) -> String {
    let s = format!("{level}");
    match level {
        Level::Error => s.red().to_string(),
        Level::Warn => s.yellow().to_string(),
        Level::Info => s.green().to_string(),
        Level::Debug => s.cyan().to_string(),
        Level::Trace => s.dimmed().to_string(),
    }
}

/// Color the path of the log message.
fn path_colored(target: &str) -> String {
    if target.contains("zahirscan") {
        target.bright_green().to_string()
    } else if target.contains("nefaxer") {
        target.bright_cyan().to_string()
    } else {
        target.magenta().to_string()
    }
}

/// Build a logger for `--snapshot-only` without the TUI.
pub fn build_logger_snapshot_only_no_tui() {
    init_cli_env_logger(log::LevelFilter::Debug);
    debug!("UBLX snapshot-only logger enabled");
}

/// Logger for headless catalog subcommands.
///
/// `query` / `doctor` stay at warn (quiet stdout). `serve` uses info so panza’s listen URL shows.
pub fn build_logger_cli_subcommand(level: log::LevelFilter) {
    init_cli_env_logger(level);
}

fn init_cli_env_logger(level: log::LevelFilter) {
    env_logger::Builder::from_default_env()
        .filter_level(level)
        .format(|buf, record| {
            let ublx = UBLX_NAMES
                .pkg_name
                .to_uppercase()
                .magenta()
                .bold()
                .to_string();
            let level = level_colored(record.level());
            let path = path_colored(record.target());
            writeln!(buf, "[{} {} {}] {}", ublx, level, path, record.args())
        })
        .init();
}

/// Format byte count as "B", "KB", "MB", "GB" etc. (uses [`KIB`], [`MIB`], [`GIB`]).
#[must_use]
pub fn format_bytes(n: u64) -> String {
    if n < KIB {
        format!("{n} B")
    } else if n < MIB {
        format!("{:.2} KB", n as f64 / KIB as f64)
    } else if n < GIB {
        format!("{:.2} MB", n as f64 / MIB as f64)
    } else {
        format!("{:.2} GB", n as f64 / GIB as f64)
    }
}

/// Unique stamp for temporary files (nanos XOR pid).
#[must_use]
pub fn unique_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        ^ (u64::from(std::process::id()) << 32)
}

/// Current Unix timestamp in nanoseconds.
#[must_use]
pub fn get_created_ns() -> i64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    i64::try_from(nanos).unwrap_or(i64::MAX)
}
