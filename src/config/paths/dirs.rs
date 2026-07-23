//! User config / cache / DB directory resolution.

use std::env;
use std::path::{Path, PathBuf};

use super::names::{UBLX_NAMES, path_to_hex};

/// User config directory for ublx. Global config lives here (e.g. `ublx.toml`).
/// - **Unix (macOS, Linux):** `~/.config/ublx`
/// - **Windows:** `%APPDATA%\ublx`
///   Returns `None` if the underlying env (e.g. `HOME`, `APPDATA`) is not set.
pub(super) fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join(UBLX_NAMES.pkg_name))
    }
    #[cfg(not(windows))]
    {
        env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config").join(UBLX_NAMES.pkg_name))
    }
}

/// User cache/data directory for ublx.
/// - **Unix (macOS, Linux):** `~/.local/share/ublx`
/// - **Windows:** `%LOCALAPPDATA%\ublx`
///   Returns `None` if the underlying env (e.g. `HOME`, `LOCALAPPDATA`) is not set.
pub(super) fn cache_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join(UBLX_NAMES.pkg_name))
    }
    #[cfg(not(windows))]
    {
        env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join(".local")
                .join("share")
                .join(UBLX_NAMES.pkg_name)
        })
    }
}

/// Per-project `SQLite` files live under `cache_dir()/ubli/` (e.g. `~/.local/share/ublx/ubli`).
#[must_use]
pub(super) fn db_dir() -> Option<PathBuf> {
    cache_dir().map(|c| c.join(UBLX_NAMES.pkg_name_plural))
}

/// Path to the global config file: `config_dir()/ublx.toml`. `None` if [`config_dir`] is unavailable.
#[must_use]
pub fn global_config_toml() -> Option<PathBuf> {
    config_dir().map(|c| c.join(UBLX_NAMES.local_config_visible_toml))
}

/// Path to the cached "last applied" config for this dir: `cache_dir()/configs/[path_hex].toml`.
/// Per-indexed-dir so global + local overlay is cached by path. Fallback when hot reload gets invalid config.
#[must_use]
pub fn last_applied_config_path(dir: &Path) -> Option<PathBuf> {
    cache_dir().map(|c| c.join("configs").join(format!("{}.toml", path_to_hex(dir))))
}

/// True if `path_str` is a relative snapshot path equal only to [`LOCAL_CONFIG_VISIBLE_TOML`] / [`LOCAL_CONFIG_HIDDEN_TOML`] at the indexed root (normalized).
#[must_use]
pub fn rel_path_is_exact_local_config_toml(path_str: &str) -> bool {
    let trim = path_str.trim();
    if Path::new(trim).is_absolute() {
        return false;
    }
    let norm = trim.replace('\\', "/");
    let norm = norm.trim_start_matches("./");
    norm == UBLX_NAMES.local_config_visible_toml || norm == UBLX_NAMES.local_config_hidden_toml
}
