//! Path resolution, global/local overlay merge, and syncing layout edit buffers from disk.

use crate::app::RunUblxParams;
use crate::config::{UblxOverlay, UblxPaths, ensure_global_config_quiet, load_ublx_toml};
use crate::layout::setup::{SettingsConfigScope, SettingsPaneState, UblxState};
use crate::themes::default_theme_for_new_config_file;
use crate::utils::opacity_is_solid;

/// Resolve which file is edited for the given scope. Local scope uses the path where the file will be
/// created on first save when no `.ublx.toml` / `ublx.toml` exists yet
/// ([`crate::config::UblxPaths::local_config_path_for_write`]).
#[must_use]
pub fn resolve_config_path(
    paths_ref: &UblxPaths,
    scope: SettingsConfigScope,
) -> Option<std::path::PathBuf> {
    match scope {
        SettingsConfigScope::Global => paths_ref.global_config(),
        SettingsConfigScope::Local => Some(paths_ref.local_config_path_for_write()),
    }
}

/// Global + local overlays and their merge (local wins where set). For Local scope UI only.
#[must_use]
pub fn local_edit_context(paths_ref: &UblxPaths) -> (Option<UblxOverlay>, UblxOverlay) {
    let global_o = load_ublx_toml(paths_ref.global_config(), None);
    let local_o = paths_ref
        .toml_path()
        .and_then(|p| load_ublx_toml(Some(p), None));
    let merged = UblxOverlay::merge(global_o, local_o.clone());
    (local_o, merged)
}

/// Effective overlay as seen before writing the file at `paths` for `scope` (local merges with global).
#[must_use]
pub fn merged_overlay_before_write(
    paths: &UblxPaths,
    scope: SettingsConfigScope,
    overlay: &UblxOverlay,
) -> UblxOverlay {
    match scope {
        SettingsConfigScope::Local => {
            let global_o = load_ublx_toml(paths.global_config(), None);
            UblxOverlay::merge(global_o, Some(overlay.clone()))
        }
        SettingsConfigScope::Global => overlay.clone(),
    }
}

/// Local scope: layout buffers follow the local file's `[layout]` when that section exists; otherwise the
/// merged global+local overlay (effective values).
#[must_use]
pub fn layout_overlay_for_local_editing<'a>(
    local: Option<&'a UblxOverlay>,
    merged: &'a UblxOverlay,
) -> &'a UblxOverlay {
    match local {
        Some(l) if l.layout.is_some() => l,
        _ => merged,
    }
}

/// `true` if `[layout]` exists in the local file.
#[must_use]
pub fn local_layout_is_explicit(local: Option<&UblxOverlay>) -> bool {
    local.is_some_and(|l| l.layout.is_some())
}

/// `true` if `bg_opacity` is set in the local file.
#[must_use]
pub fn local_opacity_is_explicit(local: Option<&UblxOverlay>) -> bool {
    local.is_some_and(|l| l.bg_opacity.is_some())
}

/// Local scope: follow local file when set; otherwise merged effective values.
#[must_use]
pub fn opacity_overlay_for_local_editing<'a>(
    local: Option<&'a UblxOverlay>,
    merged: &'a UblxOverlay,
) -> &'a UblxOverlay {
    match local {
        Some(l) if l.bg_opacity.is_some() => l,
        _ => merged,
    }
}

pub fn sync_layout_buffers_from_overlay(
    settings_mut: &mut SettingsPaneState,
    overlay_ref: &UblxOverlay,
) {
    let lo = overlay_ref.layout.clone().unwrap_or_default();
    settings_mut.layout_left_buf = lo.left_pct.to_string();
    settings_mut.layout_mid_buf = lo.middle_pct.to_string();
    settings_mut.layout_right_buf = lo.right_pct.to_string();
}

pub fn sync_opacity_buffer_from_overlay(
    settings_mut: &mut SettingsPaneState,
    overlay_ref: &UblxOverlay,
) {
    let v = overlay_ref.bg_opacity.unwrap_or(1.0);
    settings_mut.opacity_buf = format_opacity_buf(v);
}

fn format_opacity_buf(v: f32) -> String {
    if opacity_is_solid(v) {
        "1".to_string()
    } else if v == 0.0 {
        "0".to_string()
    } else {
        format!("{v:.2}")
    }
}

fn sync_layout_buffers_for_scope(
    settings_mut: &mut SettingsPaneState,
    paths_ref: &UblxPaths,
    scope: SettingsConfigScope,
) {
    match scope {
        SettingsConfigScope::Global => {
            if let Some(path) = settings_mut.editing_path.clone()
                && let Some(o) = load_ublx_toml(Some(path), None)
            {
                sync_layout_buffers_from_overlay(settings_mut, &o);
                sync_opacity_buffer_from_overlay(settings_mut, &o);
            }
        }
        SettingsConfigScope::Local => {
            let (local_o, merged) = local_edit_context(paths_ref);
            let lay_src = layout_overlay_for_local_editing(local_o.as_ref(), &merged);
            sync_layout_buffers_from_overlay(settings_mut, lay_src);
            let op_src = opacity_overlay_for_local_editing(local_o.as_ref(), &merged);
            sync_opacity_buffer_from_overlay(settings_mut, op_src);
        }
    }
}

/// Refresh `editing_path` and layout buffers from disk.
///
/// When scope is Global and the global path is known, writes the default global TOML there if missing
/// or backfills new template keys ([`ensure_global_config_quiet`]) — same behavior as TUI startup.
pub fn refresh_editing_metadata(state_mut: &mut UblxState, params_ref: &RunUblxParams<'_>) {
    let paths = UblxPaths::new(&params_ref.dir_to_ublx);
    let scope = state_mut.settings.scope;
    if scope == SettingsConfigScope::Global {
        ensure_global_config_quiet(
            &paths,
            default_theme_for_new_config_file(),
            &mut state_mut.config_written_by_us_at,
        );
    }
    state_mut.settings.editing_path = resolve_config_path(&paths, scope);
    sync_layout_buffers_for_scope(&mut state_mut.settings, &paths, scope);
}
