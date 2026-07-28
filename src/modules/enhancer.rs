//! Per-file `ZahirScan` enrichment when global `enable_enhance_all` is false.

use std::path::Path;

use crate::config::UblxOpts;
use crate::engine::db_ops;
use crate::integrations;
use crate::utils::canonicalize_dir_to_ublx;

/// Run `ZahirScan` on one file and update the snapshot row.
///
/// # Errors
///
/// Returns when zahirscan yields no output for the path or DB update fails.
pub fn enhance_single_path(
    dir_to_ublx: &Path,
    db_path: &Path,
    path_rel: &str,
    ublx_opts: &UblxOpts,
) -> Result<(), anyhow::Error> {
    let dir_abs = canonicalize_dir_to_ublx(dir_to_ublx);
    let abs = dir_abs.join(path_rel);
    let result = (|| {
        let zr = integrations::run_zahir_batch(&[abs.as_path()], ublx_opts)?;
        let map = integrations::get_zahir_output_by_path(&zr, Some(&dir_abs));
        let out = map
            .get(path_rel)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("ZahirScan produced no output for this path"))?;
        db_ops::update_snapshot_zahir_for_path(db_path, dir_to_ublx, path_rel, out)?;
        Ok::<(), anyhow::Error>(())
    })();

    if let Err(e) = db_ops::UblxCleanup::delete_ublx_tmp_files(dir_to_ublx) {
        log::debug!("remove ublx tmp after enhance: {e}");
    }
    result
}

#[cfg(feature = "tui")]
mod menu {
    use crate::app::RunUblxParams;
    use crate::config::{EnhancePolicy, UblxOpts, UblxPaths, write_local_enhance_policy};
    use crate::layout::setup::UblxState;
    use crate::modules::settings::apply_config_reload;
    use crate::ui::{UI_STRINGS, UblxAction, show_operation_toast};
    use crate::utils::clamp_selection;

    const CHOICES: usize = 2;

    /// Handle keys while the enhance-policy chooser is open. Returns true when this modal is active (consumed or not).
    pub fn handle_enhance_policy_menu(
        state: &mut UblxState,
        params: &mut RunUblxParams<'_>,
        ublx_opts: &mut UblxOpts,
        action: UblxAction,
    ) -> bool {
        if !state.enhance_policy_menu.visible {
            return false;
        }
        match action {
            UblxAction::ConfirmYes => {
                state.enhance_policy_menu.selected_index = 0;
                return handle_enhance_policy_menu(
                    state,
                    params,
                    ublx_opts,
                    UblxAction::SearchSubmit,
                );
            }
            UblxAction::ConfirmNo => {
                state.enhance_policy_menu.selected_index = 1;
                return handle_enhance_policy_menu(
                    state,
                    params,
                    ublx_opts,
                    UblxAction::SearchSubmit,
                );
            }
            UblxAction::Quit | UblxAction::SearchClear => state.close_enhance_policy_menu(),
            UblxAction::MoveDown => {
                state.enhance_policy_menu.selected_index =
                    clamp_selection(state.enhance_policy_menu.selected_index + 1, CHOICES);
            }
            UblxAction::MoveUp => {
                state.enhance_policy_menu.selected_index = clamp_selection(
                    state.enhance_policy_menu.selected_index.saturating_sub(1),
                    CHOICES,
                );
            }
            UblxAction::SearchSubmit => {
                let Some(path) = state.enhance_policy_menu.path.clone() else {
                    state.close_enhance_policy_menu();
                    return true;
                };
                let policy = if state.enhance_policy_menu.selected_index == 0 {
                    EnhancePolicy::Auto
                } else {
                    EnhancePolicy::Manual
                };
                write_local_enhance_policy(&UblxPaths::new(&params.dir_to_ublx), &path, policy);
                state.config_written_by_us_at = Some(std::time::Instant::now());
                apply_config_reload(params, ublx_opts, state, None::<&str>);
                let label = match policy {
                    EnhancePolicy::Auto => UI_STRINGS.space.enhance_policy_always,
                    EnhancePolicy::Manual => UI_STRINGS.space.enhance_policy_never,
                };
                let msg = if matches!(policy, EnhancePolicy::Auto) {
                    if state.snapshot_bg.done_received {
                        state.snapshot_bg.requested = true;
                        format!("Enhance policy: {label} — updating index (Zahir for subtree)")
                    } else {
                        state.snapshot_bg.defer_snapshot_after_current = true;
                        format!("Enhance policy: {label} — index will update after current run")
                    }
                } else {
                    format!("Enhance policy (index): {label} for subtree")
                };
                show_operation_toast(state, params, msg, "enhance-policy", log::Level::Info);
                state.close_enhance_policy_menu();
            }
            _ => {}
        }
        true
    }
}

#[cfg(feature = "tui")]
pub use menu::handle_enhance_policy_menu;
