//! Load/save [`UblxOverlay`] from TOML paths and config cache.

use std::fs;
use std::path::Path;

use log::warn;

use crate::config::paths::{UblxPaths, normalize_rel_path_for_policy};
use crate::config::theme::auto_correct_theme_name;

use super::{ColumnStatsDisplay, EnhancePolicy, EnhancePolicyEntry, LayoutOverlay, UblxOverlay};

/// When true, [`ensure_local_config_file_with_defaults`] backfills missing template keys into an
/// existing project `.ublx.toml` / `ublx.toml`. Off by default — global backfill only for now.
#[allow(dead_code)]
const BACKFILL_LOCAL_CONFIG_ON_DISK: bool = false;

/// Creates `path.parent()` when set; logs `could not create {what} …` and returns `false` on failure.
fn ensure_parent_dir(path: &Path, what: &str) -> bool {
    let Some(parent) = path.parent() else {
        return true;
    };
    if let Err(e) = fs::create_dir_all(parent) {
        warn!("could not create {what} {}: {}", parent.display(), e);
        return false;
    }
    true
}

/// Explicit defaults for a new on-disk `ublx.toml` (global or local) so the Settings tab always has a full template.
#[must_use]
pub fn default_overlay_for_new_file(default_theme_display_name: &str) -> UblxOverlay {
    UblxOverlay {
        show_hidden_files: Some(false),
        hash: Some(false),
        theme: Some(default_theme_display_name.to_string()),
        layout: Some(LayoutOverlay::default()),
        enable_enhance_all: Some(false),
        ask_enhance_on_new_root: Some(true),
        run_snapshot_on_startup: Some(true),
        typed_column_tables: Some(ColumnStatsDisplay::default()),
        ..Default::default()
    }
}

/// Create `~/.config/ublx/ublx.toml` with [`default_overlay_for_new_file`] when missing, or backfill
/// newly introduced template keys into an existing file (additive only; never overwrites user values).
pub fn ensure_global_config_file_with_defaults(
    global_path: &Path,
    default_theme_display_name: &str,
) {
    if !ensure_parent_dir(global_path, "global config parent") {
        return;
    }
    let template = default_overlay_for_new_file(default_theme_display_name);
    if global_path.exists() {
        backfill_config_file_if_needed(global_path, &template, false);
        return;
    }
    write_ublx_overlay_at(global_path, &template);
}

/// Ensure the local config file used for the indexed dir exists (visible `ublx.toml` preferred when creating).
/// When [`BACKFILL_LOCAL_CONFIG_ON_DISK`] is enabled, also backfills missing template keys into an existing file.
/// Call when opening the Settings tab so first-run (no local file) still gets an on-disk template without breaking
/// the welcome gate [`crate::config::paths::should_show_initial_prompt`] (only the `ubli/` DB file gates that flow).
pub fn ensure_local_config_file_with_defaults(paths: &UblxPaths, default_theme_display_name: &str) {
    let path = paths.toml_path().unwrap_or_else(|| paths.hidden_toml());
    if path.exists() {
        if BACKFILL_LOCAL_CONFIG_ON_DISK {
            if !ensure_parent_dir(&path, "local config parent") {
                return;
            }
            let template = default_overlay_for_new_file(default_theme_display_name);
            backfill_config_file_if_needed(&path, &template, true);
        }
        return;
    }
    if !ensure_parent_dir(&path, "local config parent") {
        return;
    }
    let overlay = default_overlay_for_new_file(default_theme_display_name);
    write_ublx_overlay_at(&path, &overlay);
}

/// When `local_scope` is true, global-only keys ([`super::strip_global_only_keys_from_local_overlay`]) are not backfilled.
fn backfill_config_file_if_needed(path: &Path, template: &UblxOverlay, local_scope: bool) {
    let Some(mut overlay) = load_ublx_toml(Some(path.to_path_buf()), None) else {
        return;
    };
    if overlay.backfill_missing_from_template(template, local_scope) {
        write_ublx_overlay_at(path, &overlay);
    }
}

#[must_use]
pub fn load_ublx_toml(
    path: Option<std::path::PathBuf>,
    valid_theme_names: Option<&[&str]>,
) -> Option<UblxOverlay> {
    let path = path?;
    let s = fs::read_to_string(&path).ok()?;
    match toml::from_str::<UblxOverlay>(&s) {
        Ok(mut overlay) => {
            let corrected_theme =
                valid_theme_names
                    .zip(overlay.theme.as_deref())
                    .and_then(|(valid, theme)| {
                        auto_correct_theme_name(theme, valid)
                            .filter(|corrected| *corrected != theme)
                    });
            if let Some(corrected) = corrected_theme {
                overlay.theme = Some(corrected.to_string());
                write_corrected_overlay(&path, &overlay);
            }
            Some(overlay)
        }
        Err(e) => {
            warn!("{}: parse error, ignoring: {}", path.display(), e);
            None
        }
    }
}

/// Write a full overlay to an arbitrary path (global or local config).
pub fn write_ublx_overlay_at(path: &Path, overlay: &UblxOverlay) {
    match toml::to_string_pretty(overlay) {
        Ok(s) => {
            if let Err(e) = fs::write(path, s) {
                warn!("could not write {}: {}", path.display(), e);
            }
        }
        Err(e) => warn!("could not serialize overlay for {}: {}", path.display(), e),
    }
}

pub fn write_corrected_overlay(path: &Path, overlay: &UblxOverlay) {
    match toml::to_string_pretty(overlay) {
        Ok(updated) => {
            if let Err(e) = fs::write(path, updated) {
                warn!(
                    "could not write corrected theme to {}: {}",
                    path.display(),
                    e
                );
            }
        }
        Err(e) => warn!(
            "could not serialize corrected overlay {}: {}",
            path.display(),
            e
        ),
    }
}

pub fn save_overlay_to_cache(ublx_paths: &UblxPaths, overlay: &UblxOverlay) {
    let Some(path) = ublx_paths.last_applied_config_path() else {
        return;
    };
    if !ensure_parent_dir(&path, "cache dir") {
        return;
    }
    match toml::to_string_pretty(overlay) {
        Ok(s) => {
            if let Err(e) = fs::write(&path, s) {
                warn!("could not write cache config {}: {}", path.display(), e);
            }
        }
        Err(e) => warn!("could not serialize overlay for cache: {e}"),
    }
}

/// Write local config with `theme = "display_name"`. Uses existing file at [`UblxPaths::toml_path`] if present, otherwise creates `.ublx.toml`. Preserves other keys from existing file or default. Logs and ignores errors.
pub fn write_local_theme(paths: &UblxPaths, theme_display_name: &str) {
    let path = paths.toml_path().unwrap_or_else(|| paths.hidden_toml());
    let mut overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
    overlay.theme = Some(theme_display_name.to_string());
    if !ensure_parent_dir(&path, "config dir") {
        return;
    }
    match toml::to_string_pretty(&overlay) {
        Ok(s) => {
            if let Err(e) = fs::write(&path, s) {
                warn!("could not write theme to {}: {}", path.display(), e);
            }
        }
        Err(e) => warn!("could not serialize overlay: {e}"),
    }
}

/// Write `.ublx.toml` in the indexed directory with only [`UblxOverlay::enable_enhance_all`] set
/// (first-run prompt, `--snapshot-only`, etc.). Visible `ublx.toml` is still read when present.
///
/// # Errors
///
/// Returns [`std::io::Error`] if TOML serialization fails or if writing [`UblxPaths::hidden_toml`] fails
/// (e.g. missing parent directory, permission denied, or disk full).
pub fn write_local_enhance_only_toml(
    ublx_paths: &UblxPaths,
    enable_enhance_all: bool,
) -> std::io::Result<()> {
    let overlay = UblxOverlay {
        enable_enhance_all: Some(enable_enhance_all),
        ..Default::default()
    };
    let s = toml::to_string(&overlay).map_err(std::io::Error::other)?;
    let path = ublx_paths.hidden_toml();
    if !ensure_parent_dir(&path, "config dir") {
        return Err(std::io::Error::other(
            "could not create parent directory for local config",
        ));
    }
    fs::write(path, s)
}

/// Merge `[[enhance_policy]]` for `rel_path` into local config (`.ublx.toml` or `ublx.toml`). Preserves other keys.
pub fn write_local_enhance_policy(paths: &UblxPaths, rel_path: &str, policy: EnhancePolicy) {
    let path = paths.toml_path().unwrap_or_else(|| paths.hidden_toml());
    let mut overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
    let mut entries = overlay.enhance_policy.unwrap_or_default();
    let norm = normalize_rel_path_for_policy(rel_path);
    entries.retain(|e| normalize_rel_path_for_policy(&e.path) != norm);
    entries.push(EnhancePolicyEntry { path: norm, policy });
    overlay.enhance_policy = Some(entries);
    if !ensure_parent_dir(&path, "config dir") {
        return;
    }
    match toml::to_string_pretty(&overlay) {
        Ok(s) => {
            if let Err(e) = fs::write(&path, s) {
                warn!(
                    "could not write enhance_policy to {}: {}",
                    path.display(),
                    e
                );
            }
        }
        Err(e) => warn!("could not serialize overlay: {e}"),
    }
}
