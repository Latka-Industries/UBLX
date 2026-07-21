//! Options for ublx, extending per-tool opts (e.g. `NefaxOpts`, zahirscan `RuntimeConfig`).
//!
//! Worker pool: [`UblxOpts::max_workers_available`] is derived from nefax tuning (drive-type aware).
//! When >= [`STREAMING_THRESHOLD`], workers are split by ratio (or overrides) across nefax, zahir, and ublx.
//! When below threshold, sequential mode: run phases one after another, each using all available workers.
//! Tokio (async TUI / right-pane resolve) uses [`TOKIO_RUNTIME_WORKERS`] via [`UblxOpts::effective_tokio_runtime_workers`] — not TOML.
//! For zahir-only (e.g. single file, no nefax), use [`UblxOpts::for_zahir_only`] with a chosen max (e.g. from tuning).
//!
//! Config overlay: global config (if present) is applied first, then local (`.ublx.toml` or `ublx.toml` in the indexed dir). Only keys present in each file override defaults. Some keys are **global-only** ([`profile::strip_global_only_keys_from_local_overlay`]); local files cannot override them.

use std::path::Path;
use std::path::PathBuf;

use crate::config::{DEFAULT_COMMAND_MODE_LEADER, overlay_command_mode_leader, profile};
use crate::integrations::{
    NefaxDriveType, NefaxOpts, ZahirOutputMode, ZahirRC, pre_opts_for_nefaxer,
};
use crate::utils::BumperBuffer;

use super::paths::{UblxPaths, normalize_rel_path_for_policy, path_is_under_or_equal};
use super::toast::OPERATION_NAME;
use super::validation;

/// Parameters for config validation and optional bumper when loading opts in [`UblxOpts::for_dir`].
pub struct UblxOptsForDirExtras<'a> {
    pub valid_theme_names: &'a [&'a str],
    pub bumper: Option<&'a BumperBuffer>,
}

/// Cached disk/tuning settings stored in the ublx DB so we can skip disk check when .ublx exists.
#[derive(Clone, Debug)]
pub struct UblxSettings {
    pub num_threads: usize,
    pub drive_type: String,
    pub parallel_walk: bool,
    /// When global config exists: "local" = use local (dir) config; "global" = use global. Stored in .ublx.
    pub config_source: Option<String>,
}

/// Parse drive type string from DB/cache ("SSD", "HDD", "Network", "Unknown").
#[must_use]
pub fn parse_drive_type(s: &str) -> NefaxDriveType {
    match s {
        "SSD" => NefaxDriveType::SSD,
        "HDD" => NefaxDriveType::HDD,
        "Network" => NefaxDriveType::Network,
        _ => NefaxDriveType::Unknown,
    }
}

fn drive_type_to_string(d: NefaxDriveType) -> &'static str {
    match d {
        NefaxDriveType::SSD => "SSD",
        NefaxDriveType::HDD => "HDD",
        NefaxDriveType::Network => "Network",
        NefaxDriveType::Unknown => "Unknown",
    }
}

/// At or above this many workers we set [`UblxOpts::streaming`] to true (callback path for nefax).
pub const STREAMING_THRESHOLD: usize = 6;

/// Tokio multi-thread runtime worker count for async TUI work (e.g. right-pane file resolve). Source of truth; not overlay/TOML.
pub const TOKIO_RUNTIME_WORKERS: usize = 2;

const HIDDEN_EXCLUDE_PATTERN: &str = ".*";

/// Options for ublx. Extends [`NefaxOpts`] and [`RuntimeConfig`]; owns worker-pool sizing and streaming.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct UblxOpts {
    /// Options passed to the nefaxer indexer (base; use [`Self::nefax_opts_with_workers`] for run).
    pub nefax_opts: NefaxOpts,
    /// `ZahirScan` runtime config. Use [`Self::zahir_runtime_config`] to get config with ublx overrides applied.
    pub zahir_rc: ZahirRC,
    /// Max workers suggested by tuning (drive-type aware). From nefax [`tuning_for_path`] in [`Self::for_dir`].
    pub max_workers_available: usize,
    /// Override workers for nefaxer. When unset and >= [`STREAMING_THRESHOLD`], uses share from 1:1:1 ratio.
    pub nefax_workers_override: Option<usize>,
    /// Override workers for zahirscan. When unset and >= threshold, uses share from ratio.
    pub zahir_workers_override: Option<usize>,
    /// Override workers for ublx (main process / other work). When unset and >= threshold, remainder from ratio.
    pub tokio_runtime_workers: usize,
    #[allow(dead_code)]
    pub ublx_workers_override: Option<usize>,
    /// Use streaming (callback) path for nefax when true.
    pub streaming: bool,
    /// When global config exists: "local" | "global". Preserved from `cached_settings` for writing back to DB.
    pub config_source: Option<String>,
    /// Theme name (e.g. "default"). From config overlay; used by `themes::get`.
    pub theme: Option<String>,
    /// Left/middle/right pane percentages (0–100). From config [layout]. Hot-reloadable.
    pub layout: profile::LayoutOverlay,
    /// Page background opacity `0.0`–`1.0` (OSC 11 + main pane reset). `None` = solid (`1.0`). Hot-reloadable.
    pub bg_opacity: Option<f32>,
    /// OSC 11 encoding when [`Self::bg_opacity`] is &lt; 1. Hot-reloadable. Global-only ([`profile::strip_global_only_keys_from_local_overlay`]).
    pub opacity_format: profile::Osc11BackgroundFormat,
    /// Editor for Open (Terminal). When None, use $EDITOR.
    pub editor_path: Option<String>,
    /// When true, run full `ZahirScan` on indexed files; when false, path-only category + space-menu enhance.
    pub enable_enhance_all: bool,
    /// When true (default), show the first-run enhance prompt for a new empty root. Set `ask_enhance_on_new_root = false` in global `ublx.toml` to skip and use `enable_enhance_all` from config instead. Global-only ([`profile::strip_global_only_keys_from_local_overlay`]).
    pub ask_enhance_on_new_root: bool,
    /// `enable_enhance_all` from the config cache **before** [`Self::for_dir`] applied the current overlay and called [`Self::save_overlay_to_cache`]. Used by snapshot `force_full` Zahir when flipping the flag to `true`.
    pub enable_enhance_all_cache_before_apply: Option<bool>,
    /// `[hash]` from the config cache **before** [`Self::for_dir`] applied the current overlay and called [`Self::save_overlay_to_cache`]. Match [`Self::enable_enhance_all_cache_before_apply`]: hot reload may set this to `Some(false)` so a snapshot `for_dir` still observes a false→true flip after the on-disk cache was updated.
    pub with_hash_cache_before_apply: Option<bool>,
    /// Effective `[[enhance_policy]]` entries (merged global + local). Used only for index-time batch Zahir.
    pub enhance_policy: Vec<profile::EnhancePolicyEntry>,
    /// When true (default), TUI startup spawns a background snapshot unless first-run defers it. Merged from global + local overlay ([`profile::UblxOverlay::merge`]).
    pub run_snapshot_on_startup: bool,
    /// Typed column tables in Metadata / Writing tabs ([`profile::ColumnStatsDisplay`]).
    pub typed_column_tables: profile::ColumnStatsDisplay,
    /// Command Mode leader letter (`Ctrl+{letter}`). Global-only. Default `a`.
    pub command_mode_leader: char,
}

impl UblxOpts {
    /// Copy hot-reloadable right-pane display fields into [`crate::layout::setup::PanelState`].
    pub fn sync_panels_display(&self, panels: &mut crate::layout::setup::PanelState) {
        panels.typed_column_tables = self.typed_column_tables;
    }

    /// Load the last applied overlay from cache (`cache_dir()/configs/[path_hex].toml`). Fallback when hot reload gets invalid config.
    #[must_use]
    pub fn load_overlay_from_cache(ublx_paths: &UblxPaths) -> Option<profile::UblxOverlay> {
        profile::load_ublx_toml(ublx_paths.last_applied_config_path(), None)
    }

    /// Load merged overlay (global then local). Same merge as [`Self::for_dir`]; used for hot reload.
    #[must_use]
    pub fn load_merged_overlay(
        ublx_paths: &UblxPaths,
        valid_theme_names: Option<&[&str]>,
    ) -> profile::UblxOverlay {
        let global = profile::load_ublx_toml(ublx_paths.global_config(), valid_theme_names);
        let local = profile::load_ublx_toml(ublx_paths.toml_path(), valid_theme_names);
        profile::UblxOverlay::merge(global, local)
    }

    /// Load overlay from a single toml file path. Returns default if path is None, file missing, or parse error.
    #[must_use]
    pub fn load_overlay_from_path(path: Option<PathBuf>) -> profile::UblxOverlay {
        profile::load_ublx_toml(path, None).unwrap_or_default()
    }

    /// Apply full overlay at startup (exclude + hot-reloadable fields). [exclude] is only applied here.
    fn apply_overlay(&mut self, overlay: &profile::UblxOverlay) {
        if let Some(ref extra) = overlay.exclude {
            self.nefax_opts.exclude.extend(extra.iter().cloned());
        }
        self.apply_hot_reload_overlay(overlay);
    }

    /// Apply only hot-reloadable fields: theme, layout, hash, `show_hidden_files`. Used when reloading config without restart.
    /// On invalid config from disk, caller should fall back to [`Self::load_overlay_from_cache`] and pass that overlay here.
    pub fn apply_hot_reload_overlay(&mut self, overlay: &profile::UblxOverlay) {
        let show_hidden = overlay.show_hidden_files.unwrap_or(false);
        if show_hidden {
            self.nefax_opts
                .exclude
                .retain(|p| p != HIDDEN_EXCLUDE_PATTERN);
            self.zahir_rc.flags.ignore_hidden_files = false;
        } else {
            if !self
                .nefax_opts
                .exclude
                .iter()
                .any(|p| p == HIDDEN_EXCLUDE_PATTERN)
            {
                self.nefax_opts
                    .exclude
                    .push(HIDDEN_EXCLUDE_PATTERN.to_string());
            }
            self.zahir_rc.flags.ignore_hidden_files = true;
        }
        if let Some(hash) = overlay.hash {
            self.nefax_opts.with_hash = hash;
        }
        if overlay.theme.is_some() {
            self.theme.clone_from(&overlay.theme);
        }
        if overlay.layout.is_some() {
            self.layout = overlay.layout.clone().unwrap_or_default();
        }
        self.bg_opacity = overlay.bg_opacity;
        self.opacity_format = overlay.opacity_format.unwrap_or_default();
        if overlay.editor_path.is_some() {
            self.editor_path.clone_from(&overlay.editor_path);
        }
        if let Some(v) = overlay.enable_enhance_all {
            self.enable_enhance_all = v;
        }
        if let Some(v) = overlay.ask_enhance_on_new_root {
            self.ask_enhance_on_new_root = v;
        }
        self.enhance_policy = overlay.enhance_policy.clone().unwrap_or_default();
        self.run_snapshot_on_startup = overlay.run_snapshot_on_startup.unwrap_or(true);
        if let Some(v) = overlay.typed_column_tables {
            self.typed_column_tables = v;
        }
        self.command_mode_leader = overlay_command_mode_leader(overlay);
    }

    /// Reload hot-reloadable config from disk (global + local merge). When disk yields no config, falls back to cached overlay from last successful load.
    /// On success, writes the merged overlay to the config cache (same path as startup `for_dir`).
    /// Validates before applying; on validation failure the overlay is not applied and errors are returned.
    /// `valid_theme_names`: allowed theme values (e.g. from [`crate::themes::theme_ordered_list`] names).
    pub fn reload_hot_config(
        &mut self,
        ublx_paths: &UblxPaths,
        valid_theme_names: &[&str],
    ) -> validation::ReloadResult {
        let from_disk = Self::load_merged_overlay(ublx_paths, Some(valid_theme_names));
        let to_apply = if from_disk == profile::UblxOverlay::default() {
            Self::load_overlay_from_cache(ublx_paths).unwrap_or_default()
        } else {
            from_disk
        };
        if to_apply == profile::UblxOverlay::default() {
            return validation::ReloadResult::default();
        }
        match validation::validate_hot_reload_overlay(&to_apply, valid_theme_names) {
            Ok(()) => {
                self.apply_hot_reload_overlay(&to_apply);
                profile::save_overlay_to_cache(ublx_paths, &to_apply);
                validation::ReloadResult {
                    applied: true,
                    validation_errors: Vec::new(),
                }
            }
            Err(validation_errors) => {
                let cached = Self::load_overlay_from_cache(ublx_paths).unwrap_or_default();
                self.apply_hot_reload_overlay(&cached);
                validation::ReloadResult {
                    applied: false,
                    validation_errors,
                }
            }
        }
    }

    /// Message for bumper when startup config is invalid and we fall back to cache.
    fn startup_validation_fallback_message(errors: &[validation::HotReloadError]) -> String {
        format!(
            "Config invalid at startup, using cache: {}",
            validation::first_validation_error_message(errors)
        )
    }

    /// Build ublx options for indexing `dir`. [`Self::max_workers_available`] comes from [`tuning_for_path`](nefaxer::tuning_for_path).
    /// Zahir config is loaded with [`RuntimeConfig::new`]. [`Self::streaming`] is set true when workers >= [`STREAMING_THRESHOLD`].
    #[must_use]
    pub fn for_dir(
        dir_to_ublx: &Path,
        ublx_paths: &UblxPaths,
        nefax_workers_override: Option<usize>,
        zahir_workers_override: Option<usize>,
        ublx_workers_override: Option<usize>,
        cached_settings: Option<&UblxSettings>,
        config: &UblxOptsForDirExtras<'_>,
    ) -> Self {
        let exclude = ublx_paths.exclude();
        let nefax_opts = pre_opts_for_nefaxer(dir_to_ublx, &exclude, cached_settings);
        let num_threads = nefax_opts.num_threads.unwrap_or(1);
        let zahir_rc = ZahirRC::new();
        let streaming = num_threads >= STREAMING_THRESHOLD;
        let config_source = cached_settings.and_then(|s| s.config_source.clone());
        let enable_enhance_all_cache_before_apply =
            Self::load_overlay_from_cache(ublx_paths).and_then(|o| o.enable_enhance_all);
        let with_hash_cache_before_apply =
            Self::load_overlay_from_cache(ublx_paths).and_then(|o| o.hash);
        let mut opts = Self {
            nefax_opts,
            zahir_rc,
            max_workers_available: num_threads,
            nefax_workers_override,
            zahir_workers_override,
            ublx_workers_override,
            tokio_runtime_workers: TOKIO_RUNTIME_WORKERS,
            streaming,
            config_source,
            theme: None,
            layout: profile::LayoutOverlay::default(),
            bg_opacity: None,
            opacity_format: profile::Osc11BackgroundFormat::default(),
            editor_path: None,
            enable_enhance_all: false,
            ask_enhance_on_new_root: true,
            enable_enhance_all_cache_before_apply,
            with_hash_cache_before_apply,
            enhance_policy: Vec::new(),
            run_snapshot_on_startup: true,
            typed_column_tables: profile::ColumnStatsDisplay::default(),
            command_mode_leader: DEFAULT_COMMAND_MODE_LEADER,
        };
        let global =
            profile::load_ublx_toml(ublx_paths.global_config(), Some(config.valid_theme_names));
        let local = profile::load_ublx_toml(ublx_paths.toml_path(), Some(config.valid_theme_names));
        let merged = profile::UblxOverlay::merge(global, local);
        match validation::validate_hot_reload_overlay(&merged, config.valid_theme_names) {
            Ok(()) => {
                opts.apply_overlay(&merged);
                profile::save_overlay_to_cache(ublx_paths, &merged);
            }
            Err(validation_errors) => {
                let cached = Self::load_overlay_from_cache(ublx_paths).unwrap_or_default();
                opts.apply_overlay(&cached);
                if let Some(b) = config.bumper {
                    b.push_with_operation(
                        log::Level::Warn,
                        Self::startup_validation_fallback_message(&validation_errors).as_str(),
                        Some(OPERATION_NAME.op("settings")),
                    );
                }
            }
        }
        opts
    }

    /// Build [`UblxSettings`] from this opts for writing to the ublx DB (so next run can skip disk check).
    pub fn to_ublx_settings(&self) -> UblxSettings {
        UblxSettings {
            num_threads: self.max_workers_available,
            drive_type: self
                .nefax_opts
                .drive_type
                .map_or("Unknown", drive_type_to_string)
                .to_string(),
            parallel_walk: self.nefax_opts.use_parallel_walk.unwrap_or(false),
            config_source: self.config_source.clone(),
        }
    }

    /// Build opts when running zahir only (e.g. single file, no nefax). You supply [`Self::max_workers_available`] (e.g. from tuning on a path); all are used for zahir.
    /// [`Self::streaming`] is set true when workers >= [`STREAMING_THRESHOLD`].
    #[must_use]
    pub fn for_zahir_only(max_workers_available: usize, zahir_rc: ZahirRC) -> Self {
        let nefax_opts = NefaxOpts::default();
        let streaming = max_workers_available >= STREAMING_THRESHOLD;
        Self {
            nefax_opts,
            zahir_rc,
            max_workers_available,
            nefax_workers_override: Some(0),
            zahir_workers_override: Some(max_workers_available),
            ublx_workers_override: Some(0),
            tokio_runtime_workers: TOKIO_RUNTIME_WORKERS,
            streaming,
            config_source: None,
            theme: None,
            layout: profile::LayoutOverlay::default(),
            bg_opacity: None,
            opacity_format: profile::Osc11BackgroundFormat::default(),
            editor_path: None,
            enable_enhance_all: true,
            ask_enhance_on_new_root: true,
            enable_enhance_all_cache_before_apply: None,
            with_hash_cache_before_apply: None,
            enhance_policy: Vec::new(),
            run_snapshot_on_startup: true,
            typed_column_tables: profile::ColumnStatsDisplay::default(),
            command_mode_leader: DEFAULT_COMMAND_MODE_LEADER,
        }
    }

    /// True when [`Self::max_workers_available`] < [`STREAMING_THRESHOLD`]: run phases sequentially, each phase using all workers.
    #[must_use]
    pub fn is_sequential_mode(&self) -> bool {
        self.max_workers_available < STREAMING_THRESHOLD
    }

    fn default_share_1_1_1(&self) -> (usize, usize, usize) {
        let n = self.max_workers_available;
        let third = n / 3;
        let nefax = third;
        let zahir = third;
        let ublx = n.saturating_sub(nefax).saturating_sub(zahir);
        (nefax, zahir, ublx)
    }

    /// Streaming: 2 workers nefaxer, 2 ublx (e.g. future live TUI), rest for zahirscan.
    fn default_share_streaming(&self) -> (usize, usize, usize) {
        let n = self.max_workers_available;
        let nefax = 2.min(n);
        let ublx = 2.min(n.saturating_sub(nefax));
        let zahir = n.saturating_sub(nefax).saturating_sub(ublx);
        (nefax, zahir, ublx)
    }

    fn default_share(&self) -> (usize, usize, usize) {
        if self.streaming {
            self.default_share_streaming()
        } else {
            self.default_share_1_1_1()
        }
    }

    fn effective_workers_for(&self, override_val: Option<usize>, default_share: usize) -> usize {
        if self.is_sequential_mode() {
            return self.max_workers_available;
        }
        override_val
            .unwrap_or(default_share)
            .min(self.max_workers_available)
    }

    /// Workers to use for nefaxer. Sequential mode: all [`Self::max_workers_available`]; else override or ratio share.
    #[must_use]
    pub fn effective_nefax_workers(&self) -> usize {
        let (n, _, _) = self.default_share();
        self.effective_workers_for(self.nefax_workers_override, n)
    }

    /// Workers to use for zahirscan. Sequential mode: all available; else override or ratio share.
    #[must_use]
    pub fn effective_zahir_workers(&self) -> usize {
        let (_, z, _) = self.default_share();
        self.effective_workers_for(self.zahir_workers_override, z)
    }

    /// Workers reserved for ublx (main process / other work). Sequential mode: all available; else override or remainder.
    #[allow(dead_code)]
    #[must_use]
    pub fn effective_ublx_workers(&self) -> usize {
        let (_, _, u) = self.default_share();
        self.effective_workers_for(self.ublx_workers_override, u)
    }

    /// Reference to the inner [`NefaxOpts`] (base, no worker override applied).
    #[must_use]
    pub fn nefax_opts(&self) -> &NefaxOpts {
        &self.nefax_opts
    }

    /// [`NefaxOpts`] with [`Self::effective_nefax_workers`] applied to `num_threads` for use with [`nefax_ops::run_nefaxer`].
    #[must_use]
    pub fn nefax_opts_with_workers(&self) -> NefaxOpts {
        let mut opts = self.nefax_opts.clone();
        opts.num_threads = Some(self.effective_nefax_workers());
        opts
    }

    /// `ZahirScan` runtime config with ublx overrides: [`OutputMode::Templates`], [`Self::effective_zahir_workers`] for `max_workers`.
    #[must_use]
    pub fn zahir_runtime_config(&self) -> ZahirRC {
        let mut config = self.zahir_rc.clone();
        config.output_mode = ZahirOutputMode::Templates;
        config.max_workers = self.effective_zahir_workers();
        config
    }

    /// Longest matching `[[enhance_policy]]` row for `rel_path`, if any; `None` means inherit [`Self::enable_enhance_all`].
    #[must_use]
    pub fn matching_enhance_policy(&self, rel_path: &str) -> Option<profile::EnhancePolicy> {
        let rel = normalize_rel_path_for_policy(rel_path);
        let mut best: Option<(usize, profile::EnhancePolicy)> = None;
        for e in &self.enhance_policy {
            let p = normalize_rel_path_for_policy(&e.path);
            if p.is_empty() {
                continue;
            }
            if path_is_under_or_equal(&rel, &p) {
                let len = p.len();
                if best.as_ref().is_none_or(|(blen, _)| len > *blen) {
                    best = Some((len, e.policy));
                }
            }
        }
        best.map(|(_, pol)| pol)
    }

    /// Whether index-time batch `ZahirScan` should run for this relative path (longest `[[enhance_policy]]` prefix wins; else [`Self::enable_enhance_all`]).
    #[must_use]
    pub fn batch_zahir_for_path(&self, rel_path: &str) -> bool {
        match self.matching_enhance_policy(rel_path) {
            Some(profile::EnhancePolicy::Auto) => true,
            Some(profile::EnhancePolicy::Manual) => false,
            None => self.enable_enhance_all,
        }
    }
}
