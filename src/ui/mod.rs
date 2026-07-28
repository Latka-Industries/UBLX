//! Keymaps, input routing, quick menus, multiselect, toasts, and string/layout constants—everything
//! that turns crossterm events into [`crate::app`] actions and drives chrome.

mod consts;
#[cfg(feature = "tui")]
mod ctrl_chord;
#[cfg(feature = "tui")]
mod file_ops;
#[cfg(feature = "tui")]
mod input;
#[cfg(feature = "tui")]
mod keymap;
#[cfg(feature = "tui")]
mod menus;
#[cfg(feature = "tui")]
mod mouse;
#[cfg(feature = "tui")]
mod multiselect;
#[cfg(feature = "tui")]
mod snapshot_toast;

#[cfg(feature = "tui")]
use crate::app::RunUblxParams;
#[cfg(feature = "tui")]
use crate::config::OPERATION_NAME;
use crate::engine::db_ops::DuplicateGroupingMode;
#[cfg(feature = "tui")]
use crate::layout::setup::UblxState;
#[cfg(feature = "tui")]
use crate::utils;

pub use consts::*;
#[cfg(feature = "tui")]
pub use ctrl_chord::*;
#[cfg(feature = "tui")]
pub use file_ops::*;
#[cfg(feature = "tui")]
pub use input::*;
#[cfg(feature = "tui")]
pub use keymap::*;
#[cfg(feature = "tui")]
pub use menus::*;
#[cfg(feature = "tui")]
pub use mouse::*;
#[cfg(feature = "tui")]
pub use snapshot_toast::*;

/// Which main tabs are available (Duplicates, Lenses). Used for key binding and mode cycle.
#[derive(Clone, Copy)]
pub struct MainTabFlags {
    pub has_duplicates: bool,
    pub has_lenses: bool,
    pub duplicate_mode: DuplicateGroupingMode,
}

/// Push a message to the bumper and refresh the stacked toast. `operation_name_suffix` is passed to `OPERATION_NAME.op` (e.g. `"lens"` → `ublx: lens`). No-op if `params.bumper` is None.
#[cfg(feature = "tui")]
pub fn show_operation_toast(
    state: &mut UblxState,
    params: &RunUblxParams<'_>,
    message: impl AsRef<str>,
    operation_name_suffix: &str,
    level: log::Level,
) {
    let op = OPERATION_NAME.op(operation_name_suffix);
    if let Some(b) = params.bumper {
        let msg = message.as_ref();
        b.push_with_operation(level, msg, Some(op.as_str()));
        utils::show_toast_slot(
            &mut state.toasts.slots,
            b,
            Some(op.as_str()),
            &mut state.toasts.consumed_per_operation,
        );
    }
}

#[cfg(test)]
mod tests;
