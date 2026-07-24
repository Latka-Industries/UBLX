//! Settings and theme picker API types.

use serde::{Deserialize, Serialize};

use super::http::{get_json, patch_json};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SettingsScope {
    #[default]
    Global,
    Local,
}

impl SettingsScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Local => "local",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Local => "Local",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct SettingsBoolControl {
    pub key: String,
    pub label: String,
    pub value: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct SettingsLayoutControl {
    pub left_pct: u16,
    pub middle_pct: u16,
    pub right_pct: u16,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub(crate) struct ThemeCssBody {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub appearance: String,
    #[serde(default)]
    pub vars: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct SettingsView {
    pub scope: String,
    pub path: String,
    pub exists: bool,
    #[serde(default)]
    pub toml: String,
    #[serde(default)]
    pub bools: Vec<SettingsBoolControl>,
    #[serde(default)]
    pub layout: SettingsLayoutControl,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub themes: Vec<String>,
    #[serde(default)]
    pub theme_picker: Vec<ThemePickerRow>,
    #[serde(default)]
    pub bg_opacity: f32,
    /// `none` | `abbrev` | `full`
    #[serde(default = "default_typed_column_tables")]
    pub typed_column_tables: String,
    #[serde(default)]
    pub css: ThemeCssBody,
}

/// Command Mode theme picker rows (`Dark` / `Light` sections + swatches).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ThemePickerRow {
    Section {
        label: String,
    },
    Theme {
        name: String,
        #[serde(default)]
        appearance: String,
        /// HSL components (`"H S% L%"`).
        #[serde(default)]
        swatch: String,
        /// Full tokens for live highlight preview.
        #[serde(default)]
        css: ThemeCssBody,
    },
}

impl ThemePickerRow {
    pub(crate) fn theme_name(&self) -> Option<&str> {
        match self {
            Self::Theme { name, .. } => Some(name.as_str()),
            Self::Section { .. } => None,
        }
    }

    pub(crate) fn theme_css(&self) -> Option<&ThemeCssBody> {
        match self {
            Self::Theme { css, .. } if !css.vars.is_empty() || !css.name.is_empty() => Some(css),
            _ => None,
        }
    }

    pub(crate) fn index_of_theme(rows: &[Self], current: &str) -> usize {
        rows.iter()
            .filter_map(Self::theme_name)
            .position(|n| n == current)
            .unwrap_or(0)
    }

    pub(crate) fn theme_name_at(rows: &[Self], index: usize) -> Option<&str> {
        rows.iter().filter_map(Self::theme_name).nth(index)
    }

    pub(crate) fn theme_css_at(rows: &[Self], index: usize) -> Option<&ThemeCssBody> {
        rows.iter().filter_map(Self::theme_css).nth(index)
    }
}

fn default_typed_column_tables() -> String {
    "abbrev".into()
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct SettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_hidden_files: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_enhance_all: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_enhance_on_new_root: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_snapshot_on_startup: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_opacity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<SettingsLayoutPatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed_column_tables: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SettingsLayoutPatch {
    pub left_pct: u16,
    pub middle_pct: u16,
    pub right_pct: u16,
}

pub(crate) async fn fetch_settings(scope: SettingsScope) -> Result<SettingsView, String> {
    get_json::<SettingsView>(&format!("/settings/{}", scope.as_str())).await
}

pub(crate) async fn patch_settings(
    scope: SettingsScope,
    patch: &SettingsPatch,
) -> Result<SettingsView, String> {
    patch_json(&format!("/settings/{}", scope.as_str()), patch).await
}
