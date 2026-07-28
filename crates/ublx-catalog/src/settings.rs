//! Cached tuning settings persisted in the `settings` table.

/// Cached disk/tuning settings stored in the ublx DB so we can skip disk check when `.ublx` exists.
///
/// Plain data: the full `UblxOpts` (overlay, theme, worker split) stays in the `ublx` binary crate and
/// builds this via `UblxOpts::to_ublx_settings`.
#[derive(Clone, Debug)]
pub struct UblxSettings {
    pub num_threads: usize,
    pub drive_type: String,
    pub parallel_walk: bool,
    /// When global config exists: "local" = use local (dir) config; "global" = use global. Stored in .ublx.
    pub config_source: Option<String>,
}
