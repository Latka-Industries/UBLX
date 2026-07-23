//! Keymaps, input routing, quick menus, multiselect, toasts, and string/layout constants—everything
//! that turns crossterm events into [`crate::app`] actions and drives chrome.

mod consts;
mod ctrl_chord;
mod file_ops;
mod input;
mod keymap;
mod menus;
mod mouse;
mod multiselect;
mod snapshot_toast;

use crate::app::RunUblxParams;
use crate::config::OPERATION_NAME;
use crate::engine::db_ops::DuplicateGroupingMode;
use crate::layout::setup::UblxState;
use crate::utils;

pub use consts::*;
pub use ctrl_chord::*;
pub use file_ops::*;
pub use input::*;
pub use keymap::*;
pub use menus::*;
pub use mouse::*;
pub use snapshot_toast::*;

/// Which main tabs are available (Duplicates, Lenses). Used for key binding and mode cycle.
#[derive(Clone, Copy)]
pub struct MainTabFlags {
    pub has_duplicates: bool,
    pub has_lenses: bool,
    pub duplicate_mode: DuplicateGroupingMode,
}

/// Push a message to the bumper and refresh the stacked toast. `operation_name_suffix` is passed to `OPERATION_NAME.op` (e.g. `"lens"` → `ublx: lens`). No-op if `params.bumper` is None.
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
