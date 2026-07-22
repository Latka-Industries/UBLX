//! HTTP settings read/patch for `ublx serve` (structured overlay; no raw TOML write).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{
    ColumnStatsDisplay, LayoutOverlay, UblxOverlay, UblxPaths, load_ublx_toml,
    strip_global_only_keys_from_local_overlay, write_ublx_overlay_at,
};
use crate::layout::setup::SettingsConfigScope;
use crate::modules::settings::{
    SettingsBoolKey, bool_key, bool_row_count, bool_row_label, overlay_bool,
    overlay_typed_column_tables, typed_column_tables_toml_value, write_bool,
    write_typed_column_tables,
};
use crate::themes;

#[derive(Debug, Clone, Serialize)]
pub struct SettingsBoolControl {
    pub key: String,
    pub label: String,
    pub value: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsLayoutControl {
    pub left_pct: u16,
    pub middle_pct: u16,
    pub right_pct: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub scope: String,
    pub path: String,
    pub exists: bool,
    /// Live file text (empty if missing).
    pub toml: String,
    pub bools: Vec<SettingsBoolControl>,
    pub layout: SettingsLayoutControl,
    /// Theme name for this scope's controls (dropdown value).
    pub theme: String,
    pub themes: Vec<String>,
    pub bg_opacity: f32,
    /// `none` | `abbrev` | `full` — display overlay value (Local = merged).
    pub typed_column_tables: String,
    /// Effective (global∪local) palette → live CSS / shadcn HSL tokens for the web UI.
    pub css: themes::ThemeCss,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SettingsPatch {
    pub show_hidden_files: Option<bool>,
    pub hash: Option<bool>,
    pub enable_enhance_all: Option<bool>,
    pub ask_enhance_on_new_root: Option<bool>,
    pub run_snapshot_on_startup: Option<bool>,
    pub theme: Option<String>,
    pub bg_opacity: Option<f32>,
    pub layout: Option<LayoutOverlay>,
    /// `none` | `abbrev` | `full`.
    pub typed_column_tables: Option<String>,
}

fn theme_names() -> Vec<&'static str> {
    themes::theme_ordered_list()
        .iter()
        .map(|p| p.name)
        .collect()
}

/// Merged global+local overlay (theme + column-stats) for serve-side rendering.
fn effective_overlay(dir: &Path, name_refs: &[&str]) -> UblxOverlay {
    let paths = UblxPaths::new(dir);
    let global = load_ublx_toml(paths.global_config(), Some(name_refs));
    let local = load_ublx_toml(Some(paths.local_config_path_for_write()), Some(name_refs));
    UblxOverlay::merge(global, local)
}

/// Merged global+local overlay theme → CSS tokens (what the web shell should look like).
fn effective_theme_css(dir: &Path, name_refs: &[&str]) -> themes::ThemeCss {
    themes::tokens_for_theme_name(effective_overlay(dir, name_refs).theme.as_deref())
}

/// Effective `typed_column_tables` for Metadata / Writing table export.
#[must_use]
pub fn effective_typed_column_tables(dir: &Path) -> ColumnStatsDisplay {
    let names: Vec<String> = theme_names().into_iter().map(str::to_string).collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    overlay_typed_column_tables(&effective_overlay(dir, &name_refs))
}

/// Effective theme appearance (dark/light) for syntect HTML in `/content`.
#[must_use]
pub fn effective_appearance(dir: &Path) -> themes::Appearance {
    let names: Vec<String> = theme_names().into_iter().map(str::to_string).collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    themes::get(effective_overlay(dir, &name_refs).theme.as_deref()).appearance
}

fn parse_typed_column_tables(raw: &str) -> Result<ColumnStatsDisplay, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(ColumnStatsDisplay::None),
        "abbrev" => Ok(ColumnStatsDisplay::Abbrev),
        "full" => Ok(ColumnStatsDisplay::Full),
        other => Err(format!(
            "invalid typed_column_tables {other:?}; expected none|abbrev|full"
        )),
    }
}

fn parse_scope(scope: &str) -> Result<SettingsConfigScope, String> {
    match scope.trim().to_ascii_lowercase().as_str() {
        "global" | "g" => Ok(SettingsConfigScope::Global),
        "local" | "l" => Ok(SettingsConfigScope::Local),
        other => Err(format!(
            "invalid settings scope {other:?}; expected global|local"
        )),
    }
}

fn scope_path(paths: &UblxPaths, scope: SettingsConfigScope) -> Option<PathBuf> {
    match scope {
        SettingsConfigScope::Global => paths.global_config(),
        SettingsConfigScope::Local => Some(paths.local_config_path_for_write()),
    }
}

fn read_toml_text(path: &Path) -> (bool, String) {
    match fs::read_to_string(path) {
        Ok(s) => (true, s),
        Err(_) => (false, String::new()),
    }
}

fn bool_description(key: &str) -> &'static str {
    match key {
        "show_hidden_files" => "Include hidden (dot) files in the catalog / index.",
        "hash" => "Compute blake3 content hashes (slower; better change detection / duplicates).",
        "enable_enhance_all" => "Run ZahirScan enrichment during snapshot for paths that need it.",
        "ask_enhance_on_new_root" => {
            "On a new root, ask before enhancing all files (global config only)."
        }
        "run_snapshot_on_startup" => "Take a snapshot when UBLX starts on this root.",
        _ => "Settings option.",
    }
}

fn bool_key_name(key: SettingsBoolKey) -> &'static str {
    match key {
        SettingsBoolKey::ShowHiddenFiles => "show_hidden_files",
        SettingsBoolKey::Hash => "hash",
        SettingsBoolKey::EnableEnhanceAll => "enable_enhance_all",
        SettingsBoolKey::AskEnhanceOnNewRoot => "ask_enhance_on_new_root",
        SettingsBoolKey::RunSnapshotOnStartup => "run_snapshot_on_startup",
    }
}

fn write_bool_by_name(
    overlay: &mut UblxOverlay,
    scope: SettingsConfigScope,
    key_name: &str,
    v: bool,
) -> Result<(), String> {
    for idx in 0..bool_row_count(scope) {
        if let Some(key) = bool_key(scope, idx)
            && bool_key_name(key) == key_name
        {
            write_bool(overlay, scope, idx, v);
            return Ok(());
        }
    }
    Err(format!("unknown bool key {key_name:?} for this scope"))
}

fn controls_from_overlay(
    scope: SettingsConfigScope,
    overlay: &UblxOverlay,
) -> (Vec<SettingsBoolControl>, SettingsLayoutControl, String, f32) {
    let mut bools = Vec::new();
    for idx in 0..bool_row_count(scope) {
        let Some(key) = bool_key(scope, idx) else {
            continue;
        };
        let key_str = bool_key_name(key).to_string();
        bools.push(SettingsBoolControl {
            key: key_str.clone(),
            label: bool_row_label(scope, idx, true).into_owned(),
            value: overlay_bool(overlay, scope, idx),
            description: bool_description(&key_str).to_string(),
        });
    }
    let layout = overlay.layout.clone().unwrap_or_default();
    (
        bools,
        SettingsLayoutControl {
            left_pct: layout.left_pct,
            middle_pct: layout.middle_pct,
            right_pct: layout.right_pct,
        },
        overlay
            .theme
            .clone()
            .unwrap_or_else(|| "Oblivion Ink".into()),
        overlay.bg_opacity.unwrap_or(1.0),
    )
}

/// Build the Settings UI payload for one scope under `dir`.
///
/// # Errors
///
/// Returns an error when `scope` is invalid or global config path cannot be resolved.
pub fn get_settings_view(dir: &Path, scope_str: &str) -> Result<SettingsView, String> {
    let scope = parse_scope(scope_str)?;
    let paths = UblxPaths::new(dir);
    let path = scope_path(&paths, scope).ok_or_else(|| {
        "could not resolve global config path (home/config dir unavailable)".to_string()
    })?;
    let names: Vec<String> = theme_names().into_iter().map(str::to_string).collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();

    let file_overlay = load_ublx_toml(Some(path.clone()), Some(name_refs.as_slice()));
    let display_overlay = match scope {
        SettingsConfigScope::Global => file_overlay.unwrap_or_default(),
        SettingsConfigScope::Local => {
            let global = load_ublx_toml(paths.global_config(), Some(name_refs.as_slice()));
            UblxOverlay::merge(global, file_overlay)
        }
    };

    let (exists, toml) = read_toml_text(&path);
    let (bools, layout, theme, bg_opacity) = controls_from_overlay(scope, &display_overlay);
    let typed_column_tables =
        typed_column_tables_toml_value(overlay_typed_column_tables(&display_overlay)).to_string();
    let css = effective_theme_css(dir, &name_refs);

    Ok(SettingsView {
        scope: match scope {
            SettingsConfigScope::Global => "global".into(),
            SettingsConfigScope::Local => "local".into(),
        },
        path: path.display().to_string(),
        exists,
        toml,
        bools,
        layout,
        theme,
        themes: names,
        bg_opacity,
        typed_column_tables,
        css,
    })
}

/// Apply a structured patch and rewrite the scope TOML. Returns the refreshed view.
///
/// # Errors
///
/// Invalid scope, missing global path, or illegal global-only keys on local.
pub fn patch_settings(
    dir: &Path,
    scope_str: &str,
    patch: SettingsPatch,
) -> Result<SettingsView, String> {
    let scope = parse_scope(scope_str)?;
    if scope == SettingsConfigScope::Local && patch.ask_enhance_on_new_root.is_some() {
        return Err("ask_enhance_on_new_root is global-only".into());
    }

    let paths = UblxPaths::new(dir);
    let path = scope_path(&paths, scope).ok_or_else(|| {
        "could not resolve global config path (home/config dir unavailable)".to_string()
    })?;
    let names: Vec<String> = theme_names().into_iter().map(str::to_string).collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();

    let mut overlay =
        load_ublx_toml(Some(path.clone()), Some(name_refs.as_slice())).unwrap_or_default();
    apply_patch(&mut overlay, scope, &patch)?;

    if scope == SettingsConfigScope::Local {
        strip_global_only_keys_from_local_overlay(&mut overlay);
    }

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    write_ublx_overlay_at(&path, &overlay);

    get_settings_view(dir, scope_str)
}

fn apply_patch(
    overlay: &mut UblxOverlay,
    scope: SettingsConfigScope,
    patch: &SettingsPatch,
) -> Result<(), String> {
    if let Some(v) = patch.show_hidden_files {
        write_bool_by_name(overlay, scope, "show_hidden_files", v)?;
    }
    if let Some(v) = patch.hash {
        write_bool_by_name(overlay, scope, "hash", v)?;
    }
    if let Some(v) = patch.enable_enhance_all {
        write_bool_by_name(overlay, scope, "enable_enhance_all", v)?;
    }
    if let Some(v) = patch.ask_enhance_on_new_root {
        write_bool_by_name(overlay, scope, "ask_enhance_on_new_root", v)?;
    }
    if let Some(v) = patch.run_snapshot_on_startup {
        write_bool_by_name(overlay, scope, "run_snapshot_on_startup", v)?;
    }
    if let Some(ref theme) = patch.theme {
        let allowed = theme_names();
        if !allowed.contains(&theme.as_str()) {
            return Err(format!("unknown theme {theme:?}"));
        }
        overlay.theme = Some(theme.clone());
    }
    if let Some(op) = patch.bg_opacity {
        if !(0.0..=1.0).contains(&op) {
            return Err("bg_opacity must be between 0 and 1".into());
        }
        overlay.bg_opacity = if (op - 1.0).abs() < f32::EPSILON {
            None
        } else {
            Some(op)
        };
    }
    if let Some(ref layout) = patch.layout {
        let sum =
            u32::from(layout.left_pct) + u32::from(layout.middle_pct) + u32::from(layout.right_pct);
        if sum != 100 {
            return Err(format!("layout percentages must sum to 100 (got {sum})"));
        }
        overlay.layout = Some(layout.clone());
    }
    if let Some(ref raw) = patch.typed_column_tables {
        let v = parse_typed_column_tables(raw)?;
        write_typed_column_tables(overlay, v);
    }
    Ok(())
}
