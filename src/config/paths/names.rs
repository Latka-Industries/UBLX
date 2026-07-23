//! Package artifact names and path-hash helpers.

use std::hash::{Hash, Hasher};
use std::path::Path;

/// Stable names for on-disk artifacts under an indexed root (`ubli/` cache, project `ublx.toml`, export dirs).
///
/// Values follow the Cargo package name where noted (`ublx` → e.g. `ublx.toml`, `.ublx`, `ublx-export/`).
pub struct UblxNames {
    /// Crate / CLI name (`CARGO_PKG_NAME`).
    pub pkg_name: &'static str,
    /// User cache directory name for per-root DB files (e.g. `ubli`).
    pub pkg_name_plural: &'static str,
    /// Index `SQLite` file extension (e.g. `.ublx`).
    pub index_db_file_ext: &'static str,
    /// Visible local config basename (e.g. `ublx.toml`).
    pub local_config_visible_toml: &'static str,
    /// Hidden local config basename (e.g. `.ublx.toml`).
    pub local_config_hidden_toml: &'static str,
    /// Nefaxer DB directory name
    pub nefax_db: &'static str,
    /// Subdirectory for Zahir JSON export (e.g. `ublx-export/`; CLI `-x` / Command Mode).
    pub zahir_export_dir_name: &'static str,
    /// Subdirectory for lens Markdown export (e.g. `ublx-lenses/`; Command Mode).
    pub lens_export_dir_name: &'static str,
}

impl Default for UblxNames {
    fn default() -> Self {
        Self::new()
    }
}

impl UblxNames {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pkg_name: env!("CARGO_PKG_NAME"),
            pkg_name_plural: "ubli",
            index_db_file_ext: concat!(".", env!("CARGO_PKG_NAME")),
            local_config_visible_toml: concat!(env!("CARGO_PKG_NAME"), ".toml"),
            local_config_hidden_toml: concat!(".", env!("CARGO_PKG_NAME"), ".toml"),
            nefax_db: ".nefaxer",
            zahir_export_dir_name: concat!(env!("CARGO_PKG_NAME"), "-export"),
            lens_export_dir_name: concat!(env!("CARGO_PKG_NAME"), "-lenses"),
        }
    }
}

pub const UBLX_NAMES: UblxNames = UblxNames::new();

/// `path_to_hex` / DB stem suffix length (16 hex chars from `DefaultHasher`).
const PATH_HASH_HEX_LEN: usize = 16;

/// True if `s` is exactly 16 ASCII hex digits (matches [`path_to_hex`] output).
#[must_use]
pub fn is_hex_hash16(s: &str) -> bool {
    s.len() == PATH_HASH_HEX_LEN && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Last segment of a DB stem after the final `_`, when it is a 16-char path hash (`{safe}_{hash}`).
#[must_use]
pub fn hash_suffix_from_db_stem(stem: &str) -> Option<&str> {
    let (_, rest) = stem.rsplit_once('_')?;
    is_hex_hash16(rest).then_some(rest)
}

/// Stable hex string for a path (for cache filenames). Same path => same string.
#[must_use]
pub fn path_to_hex(path: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(super) fn sanitize_name_for_fs(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "root".to_string()
    } else {
        trimmed.to_string()
    }
}
