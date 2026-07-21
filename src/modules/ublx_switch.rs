//! Switch indexed project: list recents-backed roots with a DB, then switch in-process to the chosen root.

use std::path::PathBuf;
use std::sync::mpsc;

use crate::app::{RunUblxParams, RunUblxStartupFlow};
use crate::config::{
    UblxOpts, UblxOptsForDirExtras, UblxPaths, all_indexed_roots_alphabetical,
    ensure_global_config_quiet, record_prior_root_selected, record_ublx_session_open,
};
use crate::engine::{db_ops, orchestrator};
use crate::handlers;
use crate::layout::setup::{self, UblxState, UblxSwitchPickerState};
use crate::themes;
use crate::ui::UblxAction;
use crate::utils::BumperBuffer;

pub fn open(state_mut: &mut UblxState, params: &RunUblxParams<'_>) {
    let roots = all_indexed_roots_alphabetical();
    let current = params.dir_to_ublx.as_path();
    let current_canon = current
        .canonicalize()
        .unwrap_or_else(|_| current.to_path_buf());
    let sel = roots
        .iter()
        .position(|p| {
            let c = p.canonicalize().unwrap_or_else(|_| p.clone());
            c == current_canon
        })
        .unwrap_or(0);
    let max = roots.len().saturating_sub(1);
    state_mut.chrome.ublx_switch = UblxSwitchPickerState {
        visible: true,
        selected_index: sel.min(max),
        roots,
    };
}

pub fn handle_key(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    action: UblxAction,
) {
    let sw = &mut state_mut.chrome.ublx_switch;
    let n = sw.roots.len();
    match action {
        UblxAction::Quit | UblxAction::SearchClear => {
            sw.visible = false;
        }
        UblxAction::MoveDown if n > 0 => {
            sw.selected_index = (sw.selected_index + 1).min(n - 1);
        }
        UblxAction::MoveUp if n > 0 => {
            sw.selected_index = sw.selected_index.saturating_sub(1);
        }
        UblxAction::SearchSubmit => {
            if n == 0 {
                sw.visible = false;
                return;
            }
            let dir = sw.roots[sw.selected_index].clone();
            let cur_c = params_mut
                .dir_to_ublx
                .canonicalize()
                .unwrap_or_else(|_| params_mut.dir_to_ublx.clone());
            let pick_c = dir.canonicalize().unwrap_or_else(|_| dir.clone());
            if pick_c == cur_c {
                sw.visible = false;
                return;
            }
            let _ = record_prior_root_selected(&dir);
            sw.visible = false;
            state_mut.session.pending_switch_to = Some(dir);
        }
        _ => {}
    }
}

/// Replace the running session’s indexed root: new DB, new opts, new background snapshot channel, fresh UI state.
///
/// Caller should clear/redraw the terminal after success if desired.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on error.
pub fn perform_session_switch<'a>(
    new_dir: PathBuf,
    params: &mut RunUblxParams<'a>,
    ublx_opts: &mut UblxOpts,
    categories: &mut Vec<String>,
    all_rows: &mut Vec<setup::TuiRow>,
    state: &mut setup::UblxState,
    bumper: Option<&'a BumperBuffer>,
) -> Result<(), String> {
    let dir = new_dir;
    let db_path = db_ops::ensure_ublx_and_db(&dir).map_err(|e| e.to_string())?;

    let cold = db_ops::load_tui_start_data(&db_path).map_err(|e| e.to_string())?;
    let paths = UblxPaths::new(&dir);
    let valid_themes: Vec<&str> = themes::theme_ordered_list()
        .iter()
        .map(|t| t.name)
        .collect();
    let for_dir_config = UblxOptsForDirExtras {
        valid_theme_names: &valid_themes,
        bumper,
    };
    *ublx_opts = UblxOpts::for_dir(
        &dir,
        &paths,
        None,
        None,
        None,
        cold.cached_settings.as_ref(),
        &for_dir_config,
    );

    let prior_opt = cold.prior_nefax;
    let c = cold.categories;
    let r = cold.rows;
    let lens_names = cold.lens_names;

    let (tx, rx) = mpsc::channel::<(usize, usize, usize)>();
    let tx_for_tui = tx.clone();
    let dir_clone = dir.clone();
    let opts_clone = ublx_opts.clone();
    let prior_for_thread = prior_opt.clone();
    let bumper_for_thread = bumper.cloned();
    let run_snapshot_on_startup = opts_clone.run_snapshot_on_startup;
    if run_snapshot_on_startup {
        std::thread::spawn(move || {
            handlers::run_snapshot_pipeline(
                &dir_clone,
                &opts_clone,
                prior_for_thread.as_ref(),
                Some(tx),
                bumper_for_thread.as_ref(),
            );
        });
    }

    let config_reload_rx = Some(handlers::spawn_config_watcher(&dir));
    let pending_force_full_enhance_toast = orchestrator::should_force_full_zahir(ublx_opts);
    let _ = record_ublx_session_open(&dir);

    let (right_pane_tx, right_pane_rx) =
        tokio::sync::mpsc::unbounded_channel::<setup::RightPaneAsyncReady>();

    params.db_path = db_path;
    params.dir_to_ublx = dir;
    params.snapshot_done_rx = Some(rx);
    params.snapshot_done_tx = Some(tx_for_tui);
    params.bumper = bumper;
    params.theme.clone_from(&ublx_opts.theme);
    params.layout = ublx_opts.layout.clone();
    params.bg_opacity = ublx_opts.bg_opacity.unwrap_or(1.0);
    params.opacity_format = ublx_opts.opacity_format;
    params.command_mode_leader = ublx_opts.command_mode_leader;
    params.duplicate_groups.clear();
    params.duplicate_mode = db_ops::DuplicateGroupingMode::NameSize;
    params.duplicate_groups_rx = None;
    params.zahir_export_rx = None;
    params.lens_export_rx = None;
    params.lens_names = lens_names;
    params.config_reload_rx = config_reload_rx;
    params.startup = RunUblxStartupFlow {
        defer_first_snapshot: false,
        pending_force_full_enhance_toast,
        skip_startup_snapshot_spawn: !ublx_opts.run_snapshot_on_startup,
    };
    params.right_pane_async_tx = Some(right_pane_tx);

    *categories = c;
    *all_rows = r;

    *state = setup::UblxState::new();
    state.right_pane_async.rx = Some(right_pane_rx);
    ensure_global_config_quiet(
        &UblxPaths::new(params.dir_to_ublx.as_path()),
        themes::default_theme_for_new_config_file(),
        &mut state.config_written_by_us_at,
    );
    state.snapshot_bg.done_received =
        !categories.is_empty() || !all_rows.is_empty() || !ublx_opts.run_snapshot_on_startup;

    Ok(())
}
