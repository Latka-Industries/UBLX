//! Background snapshot / export gates and session tick flags.

use std::path::PathBuf;

/// Background snapshot: user request, poll `.ublx_tmp` while running, and completion.
#[derive(Default)]
pub struct BackgroundSnapshot {
    pub requested: bool,
    pub poll_deadline: Option<std::time::Instant>,
    pub done_received: bool,
    /// After the in-flight snapshot finishes, run one more (e.g. `[[enhance_policy]]` = auto just saved).
    pub defer_snapshot_after_current: bool,
}

/// Lazy-load duplicate groups when the user opens the Duplicates tab.
#[derive(Default)]
pub struct DuplicateLoadGate {
    pub requested: bool,
}

/// Background flat Zahir JSON export (Command Mode + `x`).
#[derive(Default)]
pub struct ZahirExportGate {
    pub requested: bool,
}

/// Background lens Markdown export (Command Mode + `l`).
#[derive(Default)]
pub struct LensExportGate {
    pub requested: bool,
}

/// First real frame vs later ticks; redraw after returning from external editor.
#[derive(Clone, Copy, Debug)]
pub struct SessionTickFlags {
    pub first_tick: bool,
    pub refresh_terminal_after_editor: bool,
}

impl Default for SessionTickFlags {
    fn default() -> Self {
        Self {
            first_tick: true,
            refresh_terminal_after_editor: false,
        }
    }
}

/// Snapshot table reload and one-shot dedup for the background full-enhance toast.
#[derive(Clone, Copy, Debug, Default)]
pub struct SessionReloadFlags {
    /// After single-file `ZahirScan` enhance, reload snapshot rows from DB on next tick.
    pub snapshot_rows: bool,
    /// After we show the "enhancing in background" toast for [`crate::engine::orchestrator::should_force_full_zahir`], suppress duplicates until restart.
    pub force_full_enhance_toast_shown: bool,
    /// After deleting a file from Duplicates mode, reload duplicate groups from the DB on next tick.
    pub duplicate_groups: bool,
}

/// One-shot session coordination for ticks, editor handoff, and DB reload.
#[derive(Default)]
pub struct SessionFlow {
    pub tick: SessionTickFlags,
    pub reload: SessionReloadFlags,
    /// Set when the user confirms another indexed root in the project picker; next tick runs [`crate::handlers::session_switch::perform_session_switch`].
    pub pending_switch_to: Option<PathBuf>,
}
