//! Settings row for Command Mode leader letter (`[command_mode].leader` → `Ctrl+{letter}`).

use crate::config::{UblxOverlay, normalize_command_mode_leader, overlay_command_mode_leader};
use crate::layout::setup::SettingsConfigScope;
use crate::ui::UI_STRINGS;

use super::typed_column_tables_row::typed_column_tables_row_index;

/// Toast / bumper operation suffix for leader edits.
pub const LEADER_TOAST_OP: &str = "settings-command-mode-leader";

/// Row index for `command_mode.leader` (Global only), after `typed_column_tables`.
#[must_use]
pub fn command_mode_leader_row_index(scope: SettingsConfigScope) -> Option<usize> {
    match scope {
        SettingsConfigScope::Global => Some(typed_column_tables_row_index(scope) + 1),
        SettingsConfigScope::Local => None,
    }
}

#[must_use]
pub fn leader_button_label(leader: char) -> String {
    format!(" Ctrl+{} ", normalize_command_mode_leader(leader))
}

#[must_use]
pub fn command_mode_leader_row_label() -> &'static str {
    UI_STRINGS.settings_pane.command_mode_leader_label
}

/// Effective leader for display (default `a` when overlay missing).
#[must_use]
pub fn display_leader(overlay: Option<&UblxOverlay>) -> char {
    overlay.map_or(
        crate::config::DEFAULT_COMMAND_MODE_LEADER,
        overlay_command_mode_leader,
    )
}
