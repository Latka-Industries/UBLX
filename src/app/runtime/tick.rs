//! One UI tick: side effects → view build → draw → input.

use std::io;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;

use crate::app::{RunUblxParams, load_snapshot_for_tui};
use crate::config::{LayoutOverlay, UblxOpts};
use crate::engine::{db_ops, orchestrator};
use crate::handlers::{reapply_terminal_after_editor, spawn_snapshot_from_dir_db};
use crate::layout::setup;
use crate::modules;
use crate::render::marquee;
use crate::ui;
use crate::utils;

use super::frame::{DrawInputs, draw_one_frame, theme_name_for_tick};
use super::view_build::build_view_and_right_content;

const SNAPSHOT_POLL_INTERVAL: Duration = Duration::from_millis(500);

fn run_pending_session_switch(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut setup::UblxState,
    categories: &mut Vec<String>,
    all_rows: &mut Vec<setup::TuiRow>,
    params: &mut RunUblxParams<'_>,
    ublx_opts: &mut UblxOpts,
) -> io::Result<()> {
    let Some(pending) = state.session.pending_switch_to.take() else {
        return Ok(());
    };
    let bumper = params.bumper;
    match modules::ublx_switch::perform_session_switch(
        pending, params, ublx_opts, categories, all_rows, state, bumper,
    ) {
        Ok(()) => terminal.clear(),
        Err(msg) => {
            ui::show_operation_toast(state, params, msg, "switch-root", log::Level::Error);
            Ok(())
        }
    }
}

fn advance_marquees_for_tick(
    state: &mut setup::UblxState,
    term_width: u16,
    view: &setup::ViewData,
    layout: &LayoutOverlay,
    rows_for_draw: Option<&[setup::TuiRow]>,
    dir_to_ublx: &std::path::Path,
    now: Instant,
) {
    let marquee_ctx = marquee::MarqueeTickCtx {
        focus: state.panels.focus,
        main_mode: state.main_mode,
        viewer_fullscreen: state.chrome.viewer_fullscreen,
        view,
        layout,
        term_width,
        now,
    };
    marquee::tick_category_marquee_dup_lens(
        &mut state.panels.category_marquee,
        &marquee_ctx,
        state.panels.category_state.selected(),
    );
    let content_marquee_tick = marquee::ContentMarqueeTick {
        all_rows: rows_for_draw,
        dir_to_ublx: Some(dir_to_ublx),
        content_selected: state.panels.content_state.selected(),
    };
    marquee::tick_content_marquee(
        &mut state.panels.content_marquee,
        &marquee_ctx,
        &content_marquee_tick,
    );
}

/// One tick: update toasts/snapshot, build view and right content, draw, handle input. Returns true if quit requested.
pub fn run_tick(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut setup::UblxState,
    categories: &mut Vec<String>,
    all_rows: &mut Vec<setup::TuiRow>,
    params: &mut RunUblxParams<'_>,
    ublx_opts: &mut UblxOpts,
) -> io::Result<bool> {
    run_pending_session_switch(terminal, state, categories, all_rows, params, ublx_opts)?;

    tick_applets_and_io(state, categories, all_rows, params, ublx_opts);
    tick_toasts_and_snapshot(state, categories, all_rows, params, ublx_opts);

    if params.display.dev {
        utils::move_log_events();
    }

    let (view, right_content, delta_data, rows_for_draw) = build_view_and_right_content(
        state,
        categories.as_slice(),
        all_rows.as_slice(),
        params,
        ublx_opts,
    );

    let term_size = terminal.size()?;
    let now = Instant::now();
    advance_marquees_for_tick(
        state,
        term_size.width,
        &view,
        &params.layout,
        rows_for_draw,
        params.dir_to_ublx.as_path(),
        now,
    );

    ui::tick_chord_menu_timeout(state, now);

    let last_snapshot_ns = db_ops::load_delta_log_snapshot_timestamps(&params.db_path)
        .ok()
        .and_then(|v| v.into_iter().next());
    let theme_name_owned = theme_name_for_tick(state, params);
    let theme_name = theme_name_owned.as_deref();
    let has_duplicates =
        !params.duplicate_groups.is_empty() || params.duplicate_groups_rx.is_some();
    let has_lenses = !params.lens_names.is_empty();
    {
        let draw_inputs = DrawInputs {
            params,
            delta_data: delta_data.as_ref(),
            rows_for_draw,
            theme_name,
            last_snapshot_ns,
        };
        draw_one_frame(terminal, state, &view, &right_content, &draw_inputs)?;
    }
    let layout_for_input = params.layout.clone();
    let theme_ctx = modules::theme_selector::context_from_state(state, params, theme_name);
    let quit = ui::handle_ublx_input(
        state,
        ui::InputContext {
            view: &view,
            all_rows: rows_for_draw,
            right_content: &right_content,
            theme_ctx,
            frame_area: Rect::new(0, 0, term_size.width, term_size.height),
            layout: &layout_for_input,
            tabs: ui::MainTabFlags {
                has_duplicates,
                has_lenses,
                duplicate_mode: params.duplicate_mode,
            },
        },
        params,
        ublx_opts,
    )?;
    if state.session.tick.refresh_terminal_after_editor {
        state.session.tick.refresh_terminal_after_editor = false;
        reapply_terminal_after_editor()?;
        terminal.clear()?;
        let draw_inputs = DrawInputs {
            params,
            delta_data: delta_data.as_ref(),
            rows_for_draw,
            theme_name,
            last_snapshot_ns,
        };
        draw_one_frame(terminal, state, &view, &right_content, &draw_inputs)?;
    }
    Ok(quit)
}

/// Runs before view build: settings first-tick, reload flags, snapshot row reload, duplicate/export/config
/// channel drains, then spawn background dupe detection and on-disk exports if requested.
fn tick_applets_and_io(
    state: &mut setup::UblxState,
    categories: &mut Vec<String>,
    all_rows: &mut Vec<setup::TuiRow>,
    params: &mut RunUblxParams<'_>,
    ublx_opts: &mut UblxOpts,
) {
    modules::settings::on_first_tick(state, params);

    if params.startup.pending_force_full_enhance_toast {
        params.startup.pending_force_full_enhance_toast = false;
        if !state.session.reload.force_full_enhance_toast_shown {
            state.session.reload.force_full_enhance_toast_shown = true;
            ui::show_force_full_enhance_started_toast(state, params);
        }
    }

    if state.session.reload.snapshot_rows {
        let (c, r) = load_snapshot_for_tui(
            &params.db_path,
            db_ops::SnapshotReaderPreference::PreferUblx,
        );
        *categories = c;
        *all_rows = r;
        state.session.reload.snapshot_rows = false;
    }

    if state.session.reload.duplicate_groups {
        state.session.reload.duplicate_groups = false;
        match db_ops::load_duplicate_groups(
            &params.db_path,
            &params.dir_to_ublx,
            ublx_opts.nefax_opts.with_hash,
        ) {
            Ok((groups, mode)) => {
                modules::dupe_finder::prune_duplicate_ignores_after_reload(
                    &mut state.duplicate_ignored_paths,
                    &groups,
                );
                params.duplicate_groups = groups;
                params.duplicate_mode = mode;
            }
            Err(e) => log::warn!("reload duplicate groups: {e}"),
        }
    }

    if let Some(rx) = params.duplicate_groups_rx.as_ref()
        && let Ok((groups, mode)) = rx.try_recv()
    {
        params.duplicate_groups_rx = None;
        modules::dupe_finder::on_groups_received(state, params, groups, mode);
    }

    modules::exporter::zahir_poll_and_finish(state, params);
    modules::exporter::lens_poll_and_finish(state, params);

    if let Some(rx) = params.config_reload_rx.as_ref() {
        // Drain bursts (remote FS / multi-event notify) so one save → one reload.
        let mut fired = false;
        while rx.try_recv().is_ok() {
            fired = true;
        }
        if fired {
            modules::settings::on_config_reload(state, params, ublx_opts);
        }
    }

    modules::dupe_finder::spawn_if_requested(state, params, ublx_opts);
    modules::exporter::zahir_spawn_if_requested(state, params);
    modules::exporter::lens_spawn_if_requested(state, params);
}

/// Prune expired toasts, then handle snapshot request, completion, and in-progress polling (live DB reload).
fn tick_toasts_and_snapshot(
    state: &mut setup::UblxState,
    categories: &mut Vec<String>,
    all_rows: &mut Vec<setup::TuiRow>,
    params: &mut RunUblxParams<'_>,
    ublx_opts: &UblxOpts,
) {
    prune_toasts(state);
    handle_snapshot_request(state, params, ublx_opts);
    handle_snapshot_done(state, categories, all_rows, params);
    poll_snapshot_if_due(state, categories, all_rows, params);
}

fn prune_toasts(state_mut: &mut setup::UblxState) {
    state_mut
        .toasts
        .slots
        .retain(|s| Instant::now() < s.visible_until);
}

fn handle_snapshot_request(
    state: &mut setup::UblxState,
    params: &mut RunUblxParams<'_>,
    ublx_opts: &UblxOpts,
) {
    if !state.snapshot_bg.requested {
        return;
    }
    if orchestrator::should_force_full_zahir(ublx_opts)
        && !state.session.reload.force_full_enhance_toast_shown
    {
        params.startup.pending_force_full_enhance_toast = true;
    }
    spawn_snapshot_from_dir_db(
        &params.dir_to_ublx,
        &params.db_path,
        params.snapshot_done_tx.as_ref(),
        params.bumper,
        Some(ublx_opts),
    );
    state.snapshot_bg.requested = false;
    state.snapshot_bg.done_received = false; // start polling .ublx_tmp for live progress
}

fn handle_snapshot_done(
    state: &mut setup::UblxState,
    categories: &mut Vec<String>,
    all_rows: &mut Vec<setup::TuiRow>,
    params: &mut RunUblxParams<'_>,
) {
    let Some(ref rx) = params.snapshot_done_rx else {
        return;
    };
    let Ok((added, mod_count, removed)) = rx.try_recv() else {
        return;
    };

    let (c, r) = load_snapshot_for_tui(
        &params.db_path,
        db_ops::SnapshotReaderPreference::PreferUblx,
    );
    *categories = c;
    *all_rows = r;
    state.snapshot_bg.poll_deadline = None;
    state.snapshot_bg.done_received = true;
    if state.snapshot_bg.defer_snapshot_after_current {
        state.snapshot_bg.defer_snapshot_after_current = false;
        state.snapshot_bg.requested = true;
    }
    // Duplicates are lazy-loaded when user switches to that tab; don't block here.

    ui::show_snapshot_completed_toast(state, params, added, mod_count, removed);
}

/// When a background snapshot is running, periodically reload from the DB (e.g. .`ublx_tmp`) so the TUI shows live progress.
fn poll_snapshot_if_due(
    state: &mut setup::UblxState,
    categories: &mut Vec<String>,
    all_rows: &mut Vec<setup::TuiRow>,
    params: &RunUblxParams<'_>,
) {
    let Some(ref _rx) = params.snapshot_done_rx else {
        return;
    };
    if state.snapshot_bg.done_received {
        return;
    }
    let now = Instant::now();
    let due = state.snapshot_bg.poll_deadline.is_none_or(|d| now >= d);
    if !due {
        return;
    }
    let (c, r) =
        load_snapshot_for_tui(&params.db_path, db_ops::SnapshotReaderPreference::PreferTmp);
    if !c.is_empty() || !r.is_empty() {
        *categories = c;
        *all_rows = r;
    }
    state.snapshot_bg.poll_deadline = Some(now + SNAPSHOT_POLL_INTERVAL);
}
