//! Config overlay types: TOML `[layout]`, `theme`, `[[enhance_policy]]`, etc.

use serde::{Deserialize, Serialize};

/// Layout pane percentages (0–100). Used for main 3-pane split: left (categories), middle (contents), right (preview).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct LayoutOverlay {
    pub left_pct: u16,
    pub middle_pct: u16,
    pub right_pct: u16,
}

impl Default for LayoutOverlay {
    fn default() -> Self {
        Self {
            left_pct: 10,
            middle_pct: 30,
            right_pct: 60,
        }
    }
}

/// Per-directory policy for automatic (index-time) `ZahirScan`. Longest matching path prefix wins; absent entry inherits [`crate::config::UblxOpts::enable_enhance_all`].
/// Does not apply to per-file "Enhance with `ZahirScan`" from the quick actions menu (spacebar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EnhancePolicy {
    /// Index-time batch Zahir for paths under this subtree (same idea as global `enable_enhance_all` for that prefix).
    #[serde(alias = "always")]
    Auto,
    /// No batch Zahir under this subtree; enrich per file from the quick actions menu (spacebar) only.
    #[serde(alias = "never")]
    Manual,
}

/// Right-pane Zahir **column stats** tables (`columns` compact JSON): none, abbreviated, or full.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnStatsDisplay {
    /// Hide typed column-stat tables; other metadata KV rows still render.
    None,
    /// Default: name, null %, and a short stat set per type (e.g. min/max/mean for numbers).
    #[default]
    Abbrev,
    /// All stats present in the Zahir JSON.
    Full,
}

impl ColumnStatsDisplay {
    /// Typed per-column stat tables (compact `columns` JSON) are rendered when true.
    #[must_use]
    pub const fn shows_tables(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// How to encode OSC 11 background when [`UblxOverlay::bg_opacity`] &lt; 1. `WezTerm` needs **`rgba`**; some
/// terminals prefer **`hex8`** (`#RRGGBBAA`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Osc11BackgroundFormat {
    /// `rgba(r,g,b,opacity)` — `WezTerm`, many newer emulators.
    #[default]
    Rgba,
    /// `#RRGGBBAA` — e.g. Kitty-style hex+alpha.
    Hex8,
}

/// One `[[enhance_policy]]` row in `ublx.toml` / `.ublx.toml`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct EnhancePolicyEntry {
    /// Path relative to the indexed directory, using `/` separators (e.g. `photos/vacation`).
    pub path: String,
    pub policy: EnhancePolicy,
}

/// Config overlay read from config files. Only present keys override; used for global + local overlay.
/// Apply in order: defaults → global `~/.config/ublx/ublx.toml` → local `.ublx.toml` or `ublx.toml` in indexed dir.
///
/// **Global-only keys** (see [`strip_global_only_keys_from_local_overlay`]): [`Self::opacity_format`],
/// [`Self::ask_enhance_on_new_root`]. Project-local files must not override these; they are stripped before merge and when saving local TOML.
///
/// [theme], [layout], [hash], [`show_hidden_files`], [`Self::column_stats`], [`Self::run_snapshot_on_startup`], and [`UblxOverlay::bg_opacity`] are hot-reloadable; [exclude] is applied only at startup.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct UblxOverlay {
    /// Extra paths/patterns to exclude from indexing (appended to nefax [`nefaxer::NefaxOpts::exclude`]). Startup-only; not hot-reloadable.
    pub exclude: Option<Vec<String>>,
    /// When true, show hidden files; when false, exclude `.*` per segment and zahir skips hidden. Hot-reloadable.
    /// When omitted, treated as false (same as explicit false).
    #[serde(rename = "show_hidden_files")]
    pub show_hidden_files: Option<bool>,
    /// When true, nefaxer computes blake3 hash for files (slower, more accurate change detection). Hot-reloadable.
    pub hash: Option<bool>,
    /// Theme selection (e.g. "default"). Hot-reloadable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Optional [layout] section: left/middle/right pane percentages. Hot-reloadable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutOverlay>,
    /// Editor for Open (Terminal) (e.g. "vim", "nvim"). When unset, uses $EDITOR.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_path: Option<String>,
    /// When `true`, index runs full `ZahirScan` enrichment on paths that need it (normal pipeline).
    /// When `false` (default), only nefax + path-based category from `ZahirScan` file-type hints; empty `zahir_json` until per-file "Enhance with `ZahirScan`" or flip to `true` (next run re-enhances all). Hot-reloadable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_enhance_all: Option<bool>,
    /// When `false`, skip the first-run "Enhance all files" prompt for a new root; use [`Self::enable_enhance_all`] from config instead. **Global config only** (same rule as [`Self::opacity_format`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_enhance_on_new_root: Option<bool>,
    /// Optional per-path subtree rules for index-time Zahir (`[[enhance_policy]]`). Hot-reloadable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhance_policy: Option<Vec<EnhancePolicyEntry>>,
    /// Page background opacity `0.0`–`1.0` for OSC 11 + transparent main pane (`1.0` = solid, default when omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_opacity: Option<f32>,
    /// OSC 11 payload style when [`Self::bg_opacity`] &lt; 1. Default: [`Osc11BackgroundFormat::Rgba`].
    /// **Global config only** (see struct-level “global-only keys” note).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity_format: Option<Osc11BackgroundFormat>,
    /// When `true` (default), spawn a background index/snapshot when the TUI starts (if not first-run deferred). Set in global and/or local overlay; local wins on merge when both set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_snapshot_on_startup: Option<bool>,
    /// Typed column-stat tables in Metadata / Writing tabs. Global and local; local wins on merge. Default when omitted: [`ColumnStatsDisplay::Abbrev`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_stats: Option<ColumnStatsDisplay>,
}

/// Remove keys that apply only from global `ublx.toml`, so project-local merge and local file writes cannot set them.
#[inline]
pub fn strip_global_only_keys_from_local_overlay(overlay: &mut UblxOverlay) {
    overlay.opacity_format = None;
    overlay.ask_enhance_on_new_root = None;
}

impl UblxOverlay {
    /// Overlay with values from `other`; only fields set in `other` are applied (local overrides global when merging).
    pub fn merge_from(&mut self, other: &UblxOverlay) {
        if other.exclude.is_some() {
            self.exclude.clone_from(&other.exclude);
        }
        if other.show_hidden_files.is_some() {
            self.show_hidden_files = other.show_hidden_files;
        }
        if other.hash.is_some() {
            self.hash = other.hash;
        }
        if other.theme.is_some() {
            self.theme.clone_from(&other.theme);
        }
        if other.layout.is_some() {
            self.layout.clone_from(&other.layout);
        }
        if other.editor_path.is_some() {
            self.editor_path.clone_from(&other.editor_path);
        }
        if other.enable_enhance_all.is_some() {
            self.enable_enhance_all = other.enable_enhance_all;
        }
        if other.ask_enhance_on_new_root.is_some() {
            self.ask_enhance_on_new_root = other.ask_enhance_on_new_root;
        }
        if other.enhance_policy.is_some() {
            self.enhance_policy.clone_from(&other.enhance_policy);
        }
        if other.bg_opacity.is_some() {
            self.bg_opacity = other.bg_opacity;
        }
        if other.opacity_format.is_some() {
            self.opacity_format = other.opacity_format;
        }
        if other.run_snapshot_on_startup.is_some() {
            self.run_snapshot_on_startup = other.run_snapshot_on_startup;
        }
        if other.column_stats.is_some() {
            self.column_stats = other.column_stats;
        }
    }

    /// Merge global then local into one overlay (local wins for most keys). Global-only fields are taken from global only ([`strip_global_only_keys_from_local_overlay`]).
    #[must_use]
    pub fn merge(global: Option<UblxOverlay>, local: Option<UblxOverlay>) -> UblxOverlay {
        let mut out = UblxOverlay::default();
        if let Some(g) = global {
            out.merge_from(&g);
        }
        if let Some(mut l) = local {
            strip_global_only_keys_from_local_overlay(&mut l);
            out.merge_from(&l);
        }
        out
    }
}
