//! Top-level run dispatch: headless snapshot (`--snapshot-only`) or TUI with background snapshot pipeline.
//! TUI setup/teardown (terminal, raw mode) lives here; the main loop lives in [`crate::app::main_loop`].

use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crossterm::{cursor::Show as ShowCursor, event as ct_event, terminal as ct_term};
use log::debug;
use notify::{RecursiveMode, Watcher};
use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;

use crate::app;
use crate::config;
use crate::engine::db_ops::{self, TuiStartPreload};
use crate::engine::orchestrator;
use crate::handlers::snapshot_pipeline;
use crate::integrations::NefaxResult;
use crate::layout::setup;
use crate::modules::first_run;
use crate::themes::default_theme_for_new_config_file;
use crate::utils;

pub struct RunAppParamsHeadless {
    pub snapshot_only: bool,
    pub export_only: bool,
}

/// Parameters for [`run_app`]. Build after DB and opts are ready.
pub struct RunAppParams<'a> {
    pub headless: &'a RunAppParamsHeadless,
    pub dir_to_ublx: &'a Path,
    pub db_path: &'a Path,
    pub ublx_opts: &'a mut config::UblxOpts,
    pub prior_nefax: Option<&'a NefaxResult>,
    pub bumper: Option<&'a utils::BumperBuffer>,
    pub dev: bool,
    pub start_time: Option<Instant>,
    /// Show first-run welcome when [`crate::config::paths::should_show_initial_prompt`] is true (no `ubli/` DB yet).
    pub initial_prompt: bool,
    /// TUI only: categories, file rows, lens names from [`db_ops::load_tui_start_data`] (omit to load snapshot in [`run_tui_session`]).
    pub tui_start: Option<TuiStartPreload>,
}

/// First-terminal setup flags for [`run_tui_mode`].
#[derive(Clone, Copy)]
pub struct TuiModeLaunchOpts<'a> {
    pub bumper: Option<&'a utils::BumperBuffer>,
    pub dev: bool,
    pub initial_prompt: bool,
}

/// Run the app in the selected mode: `--snapshot-only` (index then exit) or TUI with background pipeline.
///
/// # Errors
///
/// Returns [`io::Error`] from headless snapshot mode, TUI setup, or the main run loop (terminal I/O).
pub fn run_app(params: &mut RunAppParams<'_>) -> std::io::Result<()> {
    if params.headless.export_only && params.headless.snapshot_only {
        headless_snap_mode(
            params.dir_to_ublx,
            params.ublx_opts,
            params.prior_nefax,
            params.start_time,
        )?;
        headless_export_zahir_mode(params.dir_to_ublx, params.db_path)
    } else if params.headless.export_only {
        headless_export_zahir_mode(params.dir_to_ublx, params.db_path)
    } else if params.headless.snapshot_only {
        headless_snap_mode(
            params.dir_to_ublx,
            params.ublx_opts,
            params.prior_nefax,
            params.start_time,
        )
    } else {
        run_tui_mode(
            params.dir_to_ublx,
            params.db_path,
            params.ublx_opts,
            params.prior_nefax,
            TuiModeLaunchOpts {
                bumper: params.bumper,
                dev: params.dev,
                initial_prompt: params.initial_prompt,
            },
            params.tui_start.take(),
        )
    }
}

fn headless_export_zahir_mode(dir_to_ublx: &Path, db_path: &Path) -> std::io::Result<()> {
    match db_ops::export_zahir_json_flat(dir_to_ublx, db_path) {
        Ok(n) => {
            let dest = dir_to_ublx.join(config::UBLX_NAMES.zahir_export_dir_name);
            if n == 0 {
                debug!(
                    "export: wrote 0 Zahir JSON files under {}; run `ublx --full-snapshot --export` for a full pass, or tune enhance policy in config",
                    dest.display()
                );
            } else {
                debug!("export: wrote {n} Zahir JSON file(s) to {}", dest.display());
            }
            Ok(())
        }
        Err(e) => {
            debug!("export failed: {e}");
            Err(std::io::Error::other(e.to_string()))
        }
    }
}

fn headless_snap_mode(
    dir_to_ublx: &Path,
    ublx_opts: &config::UblxOpts,
    prior_nefax: Option<&NefaxResult>,
    start_time: Option<Instant>,
) -> std::io::Result<()> {
    snapshot_pipeline::run_snapshot_pipeline_headless(
        dir_to_ublx,
        ublx_opts,
        prior_nefax,
        start_time,
    )
    .map_err(|e| std::io::Error::other(e.to_string()))
}

fn run_tui_mode(
    dir_to_ublx: &Path,
    db_path: &Path,
    ublx_opts: &mut config::UblxOpts,
    prior_nefax: Option<&NefaxResult>,
    launch: TuiModeLaunchOpts<'_>,
    tui_start: Option<TuiStartPreload>,
) -> std::io::Result<()> {
    let TuiModeLaunchOpts {
        bumper,
        dev,
        initial_prompt,
    } = launch;
    let (tx, rx) = mpsc::channel::<(usize, usize, usize)>();
    let tx_for_tui = tx.clone();
    if !initial_prompt && ublx_opts.run_snapshot_on_startup {
        let dir_clone = dir_to_ublx.to_path_buf();
        let opts_clone = ublx_opts.clone();
        let prior_clone = prior_nefax.cloned();
        std::thread::spawn(move || {
            snapshot_pipeline::run_snapshot_pipeline(
                &dir_clone,
                &opts_clone,
                prior_clone.as_ref(),
                Some(tx),
                None,
            );
        });
    }

    let config_reload_rx = Some(spawn_config_watcher(dir_to_ublx));

    let (lens_names, preloaded_snapshot) = if let Some(p) = tui_start {
        (p.lens_names, Some((p.categories, p.rows)))
    } else {
        (db_ops::load_lens_names(db_path).unwrap_or_default(), None)
    };
    let pending_force_full_enhance_toast =
        !initial_prompt && orchestrator::should_force_full_zahir(ublx_opts);
    if !initial_prompt {
        let _ = config::record_ublx_session_open(dir_to_ublx);
    }
    let (right_pane_tx, right_pane_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut params = app::RunUblxParams {
        db_path: db_path.to_path_buf(),
        dir_to_ublx: dir_to_ublx.to_path_buf(),
        snapshot_done_rx: Some(rx),
        snapshot_done_tx: Some(tx_for_tui),
        bumper,
        display: app::RunUblxDisplayOpts { dev },
        theme: ublx_opts.theme.clone(),
        layout: ublx_opts.layout.clone(),
        bg_opacity: ublx_opts.bg_opacity.unwrap_or(1.0),
        opacity_format: ublx_opts.opacity_format,
        duplicate_groups: Vec::new(),
        duplicate_mode: db_ops::DuplicateGroupingMode::NameSize,
        duplicate_groups_rx: None,
        zahir_export_rx: None,
        lens_export_rx: None,
        lens_names,
        config_reload_rx,
        startup: app::RunUblxStartupFlow {
            defer_first_snapshot: initial_prompt,
            pending_force_full_enhance_toast,
            skip_startup_snapshot_spawn: !initial_prompt && !ublx_opts.run_snapshot_on_startup,
        },
        right_pane_async_tx: Some(right_pane_tx),
    };
    run_tui_session(
        &mut params,
        ublx_opts,
        Some(right_pane_rx),
        preloaded_snapshot,
    )?;
    if let Some(b) = bumper
        && dev
    {
        utils::flush_bumper_to_stderr(b);
    }
    Ok(())
}

/// Restore terminal to cooked mode, leave alternate screen, show cursor. Used on normal exit and from panic hook.
pub fn restore_terminal() {
    let _ = ct_term::disable_raw_mode();
    let mut out = io::stdout();
    let _ = crossterm::execute!(
        out,
        ct_event::DisableMouseCapture,
        ct_term::LeaveAlternateScreen,
        ShowCursor
    );
    let _ = utils::reset_osc_dynamic_background(&mut out);
}

/// Replace this process with `ublx` running on `dir` (same as `ublx <dir>` on the command line).
pub fn relaunch_ublx_indexed_dir(dir: &Path) -> ! {
    restore_terminal();
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ublx"));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = process::Command::new(&exe).arg(dir).exec();
        eprintln!("ublx: failed to relaunch: {err}");
        process::exit(126);
    }
    #[cfg(not(unix))]
    {
        let status = process::Command::new(&exe).arg(dir).status();
        process::exit(status.map(|s| s.code().unwrap_or(1)).unwrap_or(1));
    }
}

/// Leave alternate screen and raw mode so an external editor runs on the main screen; call before spawning the editor.
///
/// # Errors
///
/// Returns [`io::Error`] from crossterm when disabling raw mode or manipulating the terminal.
pub fn leave_terminal_for_editor() -> io::Result<()> {
    ct_term::disable_raw_mode()?;
    let mut out = io::stdout();
    crossterm::execute!(
        out,
        ct_event::DisableMouseCapture,
        ct_term::LeaveAlternateScreen,
        ShowCursor
    )?;
    Ok(())
}

/// Re-enter alternate screen and raw mode after the editor exits, so the TUI can redraw.
///
/// # Errors
///
/// Returns [`io::Error`] from crossterm when enabling raw mode or manipulating the terminal.
pub fn reapply_terminal_after_editor() -> io::Result<()> {
    ct_term::enable_raw_mode()?;
    let mut out = io::stdout();
    crossterm::execute!(
        out,
        ct_term::EnterAlternateScreen,
        ct_event::EnableMouseCapture
    )?;
    Ok(())
}

/// Enter TUI terminal state, run [`crate::app::main_loop`], then restore the shell. Called by [`run_tui_mode`]
/// after channels and params are wired; use [`run_app`] for the headless vs TUI dispatch.
/// A panic hook restores the terminal on panic so the shell stays usable.
///
/// # Errors
///
/// Returns [`io::Error`] from terminal setup, the main loop, or teardown (raw mode, alternate screen, draw).
pub fn run_tui_session(
    params: &mut app::RunUblxParams<'_>,
    ublx_opts: &mut config::UblxOpts,
    right_pane_async_rx: Option<tokio::sync::mpsc::UnboundedReceiver<setup::RightPaneAsyncReady>>,
    preloaded_snapshot: Option<(Vec<String>, Vec<setup::TuiRow>)>,
) -> io::Result<()> {
    let (mut categories, mut all_rows) = if let Some((c, r)) = preloaded_snapshot {
        (c, r)
    } else {
        app::load_snapshot_for_tui(
            &params.db_path,
            db_ops::SnapshotReaderPreference::PreferUblx,
        )
    };
    let mut state = setup::UblxState::new();
    ublx_opts.sync_panels_display(&mut state.panels);
    state.right_pane_async.rx = right_pane_async_rx;
    config::ensure_global_config_quiet(
        &config::UblxPaths::new(params.dir_to_ublx.as_path()),
        default_theme_for_new_config_file(),
        &mut state.config_written_by_us_at,
    );
    if params.startup.defer_first_snapshot {
        first_run::init_prompt_state(&mut state, params.dir_to_ublx.as_path());
    }
    debug!(
        "clipboard copy command: {}",
        state
            .clipboard_copy
            .as_ref()
            .map_or_else(|| "(none)".to_owned(), |c| c.argv.join(" "))
    );
    // Already-done dir: we have data, skip polling to avoid redundant first-tick load (stutter).
    if !categories.is_empty() || !all_rows.is_empty() {
        state.snapshot_bg.done_received = true;
    }
    // First-run prompt defers the background snapshot; treat as idle for scheduling (not "snapshot in flight").
    if params.startup.defer_first_snapshot {
        state.snapshot_bg.done_received = true;
    }
    if params.startup.skip_startup_snapshot_spawn {
        state.snapshot_bg.done_received = true;
    }

    ct_term::enable_raw_mode()?;
    let mut out = io::stdout();
    crossterm::execute!(
        out,
        ct_term::EnterAlternateScreen,
        ct_event::EnableMouseCapture
    )?;
    let _ = utils::sync_osc11_page_background(
        params.theme.as_deref(),
        params.bg_opacity,
        params.opacity_format,
    );
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    let result = app::main_loop(
        &mut terminal,
        &mut state,
        &mut categories,
        &mut all_rows,
        params,
        ublx_opts,
    );

    restore_terminal();
    terminal.show_cursor()?;
    result
}

/// Debounce window: only send at most one reload signal per this many ms (avoids triplicate from multiple notify events per save).
const CONFIG_WATCH_DEBOUNCE_MS: u64 = 600;
/// Watcher thread parks for this many seconds when idle (long sleep so the thread stays alive without busy-looping).
const CONFIG_WATCHER_PARK_SECS: u64 = 86400;

/// Spawns a thread that watches global and local config paths; sends `()` when a config file changes so the main loop can trigger hot reload. Debounced so one save yields one signal. If the watcher cannot be created, the thread exits silently (no reload signals).
#[must_use]
pub fn spawn_config_watcher(dir_to_ublx: &Path) -> mpsc::Receiver<()> {
    let paths = config::UblxPaths::new(dir_to_ublx);
    let global = paths.global_config();
    let dir = dir_to_ublx.to_path_buf();
    let (tx, rx) = mpsc::channel();
    let last_send_ms = Arc::new(AtomicU64::new(0));

    std::thread::spawn(move || {
        let paths = config::UblxPaths::new(&dir);
        let global_clone = global.clone();
        let last_send = Arc::clone(&last_send_ms);
        let Ok(mut watcher) = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(e) = res {
                for p in &e.paths {
                    let is_config =
                        paths.is_config_file(p) || global_clone.as_ref().is_some_and(|g| g == p);
                    if is_config {
                        let now_ms = u64::try_from(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis(),
                        )
                        .unwrap_or(u64::MAX);
                        if now_ms.saturating_sub(last_send.load(Ordering::Relaxed))
                            > CONFIG_WATCH_DEBOUNCE_MS
                        {
                            last_send.store(now_ms, Ordering::Relaxed);
                            let _ = tx.send(());
                        }
                        break;
                    }
                }
            }
        }) else {
            return;
        };
        if watcher.watch(&dir, RecursiveMode::NonRecursive).is_err() {
            return;
        }
        if let Some(ref g) = global {
            let _ = watcher.watch(g, RecursiveMode::NonRecursive);
        }
        loop {
            std::thread::sleep(std::time::Duration::from_secs(CONFIG_WATCHER_PARK_SECS));
        }
    });

    rx
}
