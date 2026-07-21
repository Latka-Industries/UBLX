use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;
use log::debug;

use ublx::cli;
use ublx::cli_parser;
use ublx::config;
use ublx::engine::db_ops;
use ublx::handlers;
use ublx::integrations::{NefaxResult, load_prior_nefax_or_exit};
use ublx::themes;
use ublx::utils;

/// When not headless, allocates a toast bumper and wires logging to it. Headless snapshot/export
/// uses logging only (no TUI bumper).
fn setup_bumper(headless_status: bool, dev: bool) -> Option<utils::BumperBuffer> {
    if headless_status {
        utils::build_logger_snapshot_only_no_tui();
        None
    } else {
        let b = utils::BumperBuffer::new(config::TOAST_CONFIG.bumper_cap_for(dev));
        utils::init_logging(b.clone(), dev);
        Some(b)
    }
}

/// Loads Nefax snapshot data, optional TUI preload, and cached settings from the DB according to
/// headless mode: snapshot path loads prior Nefax + settings; export-only loads settings; TUI loads
/// the combined [`db_ops::load_tui_start_data`] pass.
fn load_startup_db_data(
    headless_mode_flags: cli_parser::HeadlessModeFlags,
    dir_to_ublx_ref: &Path,
    db_path_ref: &Path,
) -> (
    Option<NefaxResult>,
    Option<db_ops::TuiStartPreload>,
    Option<config::UblxSettings>,
) {
    let (prior_nefax_owned, tui_start, cached_settings) = if headless_mode_flags.snapshot.is_some()
    {
        (
            load_prior_nefax_or_exit(dir_to_ublx_ref, db_path_ref),
            None,
            db_ops::load_settings_from_db(db_path_ref).ok().flatten(),
        )
    } else if headless_mode_flags.export {
        (
            None,
            None,
            db_ops::load_settings_from_db(db_path_ref).ok().flatten(),
        )
    } else {
        let load = utils::fatal_error_handler(
            db_ops::load_tui_start_data(db_path_ref),
            "failed to load snapshot: {}",
        );
        let (prior, preload, cached) = load.split_for_app();
        (prior, Some(preload), cached)
    };
    (prior_nefax_owned, tui_start, cached_settings)
}

/// Resolves the indexing directory, installs the panic hook, builds [`config::UblxPaths`], records
/// whether a DB already existed, and ensures the ublx tree and `SQLite` DB exist.
fn setup_workspace_and_db(
    args: &cli_parser::Args,
) -> (PathBuf, config::UblxPaths, PathBuf, bool, bool) {
    let dir_to_ublx = utils::validate_dir(&args.dir_to_ublx);
    utils::install_panic_hook_with_ublx_log(&dir_to_ublx);
    debug!("indexing directory: {}", dir_to_ublx.display());

    let paths = config::UblxPaths::new(&dir_to_ublx);
    let had_index_db_before_ensure = paths.db().exists();
    let had_any_cached_db_before_this_root = config::has_any_cached_ublx_db();

    let db_path = utils::fatal_error_handler(
        db_ops::ensure_ublx_and_db(&dir_to_ublx),
        "failed to set up ublx db: {}",
    );
    debug!("db: {}", db_path.display());

    (
        dir_to_ublx,
        paths,
        db_path,
        had_index_db_before_ensure,
        had_any_cached_db_before_this_root,
    )
}

/// Builds [`config::UblxOpts`] for the current root (theme list, bumper, optional DB-backed
/// settings). For snapshot runs with no local TOML, applies `local_enhance_all` to
/// [`config::UblxOpts::enable_enhance_all`].
fn build_ublx_opts(
    dir_to_ublx: &Path,
    paths: &config::UblxPaths,
    cached_settings: Option<&config::UblxSettings>,
    bumper: Option<&utils::BumperBuffer>,
    snapshot_mode: bool,
    local_enhance_all: bool,
) -> config::UblxOpts {
    let valid_themes: Vec<&str> = themes::theme_ordered_list()
        .iter()
        .map(|t| t.name)
        .collect();
    let for_dir_config = ublx::config::UblxOptsForDirExtras {
        valid_theme_names: &valid_themes,
        bumper,
    };
    let mut ublx_opts = config::UblxOpts::for_dir(
        dir_to_ublx,
        paths,
        None,
        None,
        None,
        cached_settings,
        &for_dir_config,
    );

    if snapshot_mode && paths.toml_path().is_none() {
        ublx_opts.enable_enhance_all = local_enhance_all;
    }

    ublx_opts
}

/// Inputs for [`run_app_and_postprocess`]: paths, opts, DB preload, and run flags.
struct RunAppInvocation<'a> {
    headless: &'a cli_parser::HeadlessModeFlags,
    dir_to_ublx: &'a Path,
    db_path: &'a Path,
    paths: &'a config::UblxPaths,
    ublx_opts: &'a mut config::UblxOpts,
    prior_nefax_owned: Option<NefaxResult>,
    tui_start: Option<db_ops::TuiStartPreload>,
    bumper: Option<&'a utils::BumperBuffer>,
    dev: bool,
    start_time: Option<Instant>,
    initial_prompt: bool,
    local_enhance_all: bool,
}

/// Runs the app (TUI or headless). After a snapshot-only run with no local config file, writes
/// `ubli.toml` with the enhance-all flag so the next TUI session matches CLI intent.
fn run_app_and_postprocess(inv: RunAppInvocation<'_>) {
    let headless_params = handlers::RunAppParamsHeadless {
        snapshot_only: inv.headless.snapshot.is_some(),
        export_only: inv.headless.export,
    };
    let mut run_params = handlers::RunAppParams {
        headless: &headless_params,
        dir_to_ublx: inv.dir_to_ublx,
        db_path: inv.db_path,
        ublx_opts: inv.ublx_opts,
        prior_nefax: inv.prior_nefax_owned.as_ref(),
        tui_start: inv.tui_start,
        bumper: inv.bumper,
        dev: inv.dev,
        start_time: inv.start_time,
        initial_prompt: inv.initial_prompt,
    };
    utils::fatal_error_handler(handlers::run_app(&mut run_params), "{}");

    if inv.headless.snapshot.is_some() && inv.paths.toml_path().is_none() {
        utils::fatal_error_handler(
            config::write_local_enhance_only_toml(inv.paths, inv.local_enhance_all),
            "failed to write local config: {}",
        );
    }
}

/// Parses CLI, wires logging and DB, builds opts, and runs or exits (e.g. `--themes`).
fn main() {
    let args = cli_parser::Args::parse();
    if args.themes {
        cli_parser::print_available_themes();
        return;
    }

    if let Some(ref command) = args.command {
        let log_level = match command {
            cli_parser::Commands::Serve(_) => log::LevelFilter::Info,
            _ => log::LevelFilter::Warn,
        };
        utils::build_logger_cli_subcommand(log_level);
        utils::fatal_error_handler(cli::run(command), "{}");
        return;
    }

    let headless_mode_flags = cli_parser::headless_handler(&args.headless);
    let start_time = headless_mode_flags.snapshot.is_some().then(Instant::now);
    let bumper = setup_bumper(headless_mode_flags.is_headless(), args.dev);
    let local_enhance_all = headless_mode_flags.determine_enhance_all();

    let (
        dir_to_ublx,
        paths,
        db_path,
        had_index_db_before_ensure,
        had_any_cached_db_before_this_root,
    ) = setup_workspace_and_db(&args);

    let initial_prompt = config::should_show_initial_prompt(
        headless_mode_flags.snapshot.is_some(),
        had_index_db_before_ensure,
        had_any_cached_db_before_this_root,
    );

    let (prior_nefax_owned, tui_start, cached_settings) =
        load_startup_db_data(headless_mode_flags, &dir_to_ublx, &db_path);

    if cached_settings.is_some() {
        debug!("using cached settings from ublx db (skipping disk check)");
    }

    let mut ublx_opts = build_ublx_opts(
        &dir_to_ublx,
        &paths,
        cached_settings.as_ref(),
        bumper.as_ref(),
        headless_mode_flags.snapshot.is_some(),
        local_enhance_all,
    );

    debug!("UBLX CONFIG: {ublx_opts:#?}");

    run_app_and_postprocess(RunAppInvocation {
        headless: &headless_mode_flags,
        dir_to_ublx: &dir_to_ublx,
        db_path: &db_path,
        paths: &paths,
        ublx_opts: &mut ublx_opts,
        prior_nefax_owned,
        tui_start,
        bumper: bumper.as_ref(),
        dev: args.dev,
        start_time,
        initial_prompt,
        local_enhance_all,
    });
}
