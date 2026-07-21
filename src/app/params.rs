//! Parameters for the TUI app loop.

use std::path::PathBuf;
use std::sync::mpsc;

use tokio::sync::mpsc::UnboundedSender;

use crate::config::{LayoutOverlay, Osc11BackgroundFormat};
use crate::engine::db_ops::{DuplicateGroup, DuplicateGroupingMode};
use crate::layout::setup::RightPaneAsyncReady;
use crate::utils;

/// Dev logging (from CLI / config).
#[derive(Clone, Copy, Debug, Default)]
pub struct RunUblxDisplayOpts {
    pub dev: bool,
}

/// First-run enhance prompt and deferred “full Zahir in background” toast.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunUblxStartupFlow {
    /// First launch in this dir: show enhance-all prompt; defer first background snapshot until answered.
    pub defer_first_snapshot: bool,
    /// When set, next tick shows a toast that index-time Zahir is running in the background ([`crate::engine::orchestrator::should_force_full_zahir`]).
    pub pending_force_full_enhance_toast: bool,
    /// When true, startup did not spawn a background snapshot ([`crate::config::UblxOpts::run_snapshot_on_startup`] false); treat snapshot as idle so the status line does not wait on a completion channel.
    pub skip_startup_snapshot_spawn: bool,
}

/// Parameters for the TUI event loop. Passed from [`crate::handlers::core::run_tui_session`] into [`super::main_loop`].
pub struct RunUblxParams<'a> {
    pub db_path: PathBuf,
    pub dir_to_ublx: PathBuf,
    pub snapshot_done_rx: Option<mpsc::Receiver<(usize, usize, usize)>>,
    pub snapshot_done_tx: Option<mpsc::Sender<(usize, usize, usize)>>,
    pub bumper: Option<&'a utils::BumperBuffer>,
    pub display: RunUblxDisplayOpts,
    pub theme: Option<String>,
    /// Left/middle/right pane percentages (0–100). Hot-reloadable from config [layout].
    pub layout: LayoutOverlay,
    /// Effective page background opacity (`1.0` = solid). From [`crate::config::UblxOpts::bg_opacity`].
    pub bg_opacity: f32,
    /// OSC 11 payload style. From [`crate::config::UblxOpts::opacity_format`].
    pub opacity_format: Osc11BackgroundFormat,
    /// Command Mode leader letter. From [`crate::config::UblxOpts::command_mode_leader`].
    pub command_mode_leader: char,
    /// Duplicate groups (lazy-loaded when user switches to Duplicates tab). Empty until load completes.
    pub duplicate_groups: Vec<DuplicateGroup>,
    /// Grouping mode backing `duplicate_groups` (Hash vs Name+Size fallback label).
    pub duplicate_mode: DuplicateGroupingMode,
    /// When some, a background thread is loading duplicate groups; main loop receives via `try_recv`.
    pub duplicate_groups_rx: Option<mpsc::Receiver<(Vec<DuplicateGroup>, DuplicateGroupingMode)>>,
    /// When some, background [`crate::engine::db_ops::export_zahir_json_flat`] is running; main loop receives via `try_recv`.
    pub zahir_export_rx: Option<mpsc::Receiver<Result<usize, String>>>,
    /// When some, background [`crate::engine::db_ops::export_lenses_markdown_flat`] is running; main loop receives via `try_recv`.
    pub lens_export_rx: Option<mpsc::Receiver<Result<usize, String>>>,
    /// Lens names for the Lenses tab (loaded at startup). When non-empty, Lenses tab is shown.
    pub lens_names: Vec<String>,
    /// When some, a file watcher sends () on global/local config save; main loop triggers hot reload.
    pub config_reload_rx: Option<mpsc::Receiver<()>>,
    pub startup: RunUblxStartupFlow,
    /// When set, file-row right-pane content is resolved on a background worker.
    pub right_pane_async_tx: Option<UnboundedSender<RightPaneAsyncReady>>,
}
