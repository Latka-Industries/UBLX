use log::{debug, error};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use rayon::prelude::*;

use crate::config::{PARALLEL, RunMode, UblxOpts, UblxPaths};
use crate::engine::db_ops;
use crate::integrations;
use crate::utils;

type PreRunSetup = (PathBuf, HashMap<String, String>, HashMap<String, String>);

fn pre_run_setup(dir_to_ublx: &Path) -> PreRunSetup {
    let dir_to_ublx_abs = utils::canonicalize_dir_to_ublx(dir_to_ublx);
    let db_path = UblxPaths::new(dir_to_ublx).db();
    let prior_zahir_json = db_ops::load_snapshot_zahir_json_map(&db_path).unwrap_or_default();
    let prior_category = db_ops::load_snapshot_category_map(&db_path).unwrap_or_default();
    (dir_to_ublx_abs, prior_zahir_json, prior_category)
}

/// Enhance-policy predicate handed to `db_ops` so the catalog crate never sees [`UblxOpts`].
#[inline]
fn batch_zahir_predicate(ublx_opts: &UblxOpts) -> impl Fn(&str) -> bool + Sync {
    move |rel_path: &str| ublx_opts.batch_zahir_for_path(rel_path)
}

#[inline]
fn snapshot_prior_ctx<'a>(
    prior_zahir_json: &'a HashMap<String, String>,
    prior_category: &'a HashMap<String, String>,
    batch_zahir_for_path: &'a (dyn Fn(&str) -> bool + Sync),
) -> db_ops::SnapshotPriorContext<'a> {
    db_ops::SnapshotPriorContext {
        prior_zahir_json,
        prior_category,
        batch_zahir_for_path,
    }
}

/// Log nefax error and exit. Use in both sequential and stream error paths.
fn on_nefax_error(dir_to_ublx: &Path, e: &impl std::fmt::Display) -> ! {
    let _ = utils::write_nefax_error_to_log(dir_to_ublx, e);
    error!("{}", utils::NefaxZahirErrors::nefax_failed(e));
    utils::exit_error();
}

/// Run nefaxer; on success return `(nefax, diff)`, on error log and exit. Use when no cleanup is needed (e.g. sequential).
fn run_nefax_exiting<F>(
    dir_to_ublx: &Path,
    ublx_opts: &UblxOpts,
    prior_nefax: Option<&integrations::NefaxResult>,
    entry_callback: Option<F>,
) -> (integrations::NefaxResult, integrations::NefaxDiff)
where
    F: FnMut(&integrations::NefaxEntry),
{
    match integrations::run_nefaxer(dir_to_ublx, ublx_opts, prior_nefax, entry_callback) {
        Ok(result) => result,
        Err(e) => on_nefax_error(dir_to_ublx, &e),
    }
}

/// True when this index run should invoke Zahir for the path: mtime/new-file, global force-full, or no usable prior `zahir_json` in the DB (e.g. new `[[enhance_policy]]` = auto on a subtree that was previously path-hint-only).
#[inline]
fn path_needs_zahir_extract(
    force_full_zahir: bool,
    prior_nefax: Option<&integrations::NefaxResult>,
    path: &PathBuf,
    mtime_ns: i64,
    rel_str: &str,
    prior_zahir_json: &HashMap<String, String>,
) -> bool {
    force_full_zahir
        || integrations::needs_zahir(prior_nefax, path, mtime_ns)
        || prior_zahir_json.get(rel_str).is_none_or(String::is_empty)
}

/// Shared inputs for [`entry_should_batch_zahir`] (sequential + streaming).
struct BatchZahirCtx<'a> {
    dir_to_ublx_abs: &'a Path,
    ublx_opts: &'a UblxOpts,
    force_full_zahir: bool,
    prior_nefax: Option<&'a integrations::NefaxResult>,
    prior_zahir_json: &'a HashMap<String, String>,
}

/// True when this indexed path should be sent to index-time batch Zahir (sequential list or stream channel).
fn entry_should_batch_zahir(
    ctx: &BatchZahirCtx<'_>,
    path: &PathBuf,
    size: u64,
    mtime_ns: i64,
) -> bool {
    if size == 0 {
        return false;
    }
    let rel_str = utils::path_to_slash_string(path);
    if !ctx.ublx_opts.batch_zahir_for_path(&rel_str) {
        return false;
    }
    if utils::rel_path_is_directory(ctx.dir_to_ublx_abs, path)
        && !integrations::is_zarr_store_root_rel_path(path)
    {
        return false;
    }
    path_needs_zahir_extract(
        ctx.force_full_zahir,
        ctx.prior_nefax,
        path,
        mtime_ns,
        rel_str.as_str(),
        ctx.prior_zahir_json,
    )
}

/// Index-time batch `ZahirScan`: paths that pass [`UblxOpts::batch_zahir_for_path`] and still need a Zahir extract (see [`path_needs_zahir_extract`]).
fn paths_needing_zahir(
    nefax: &integrations::NefaxResult,
    prior_nefax: Option<&integrations::NefaxResult>,
    dir_to_ublx_abs: &Path,
    ublx_opts: &UblxOpts,
    force_full_zahir: bool,
    prior_zahir_json: &HashMap<String, String>,
) -> Vec<PathBuf> {
    let ctx = BatchZahirCtx {
        dir_to_ublx_abs,
        ublx_opts,
        force_full_zahir,
        prior_nefax,
        prior_zahir_json,
    };
    let entries: Vec<_> = nefax.iter().collect();
    let filter_map = |(path, meta): &(&PathBuf, &integrations::NefaxPathMeta)| {
        if !entry_should_batch_zahir(&ctx, path, meta.size, meta.mtime_ns) {
            return None;
        }
        Some(dir_to_ublx_abs.join(path))
    };
    if entries.len() >= PARALLEL.paths_needing_zahir {
        entries.par_iter().filter_map(filter_map).collect()
    } else {
        entries.iter().filter_map(filter_map).collect()
    }
}

/// True when `enable_enhance_all` just flipped to `true` vs the value that was in the config cache before this process applied the merged overlay (see [`UblxOpts::enable_enhance_all_cache_before_apply`]).
#[must_use]
pub fn should_force_full_zahir(ublx_opts: &UblxOpts) -> bool {
    if !ublx_opts.enable_enhance_all {
        return false;
    }
    let prev = ublx_opts
        .enable_enhance_all_cache_before_apply
        .unwrap_or(false);
    ublx_opts.enable_enhance_all && !prev
}

/// Run the index → zahir pipeline. Mode (sequential vs stream) is derived from [`UblxOpts`].
/// Returns (added, modified, removed) counts for this run.
///
/// # Errors
///
/// Returns [`std::io::Error`] only from I/O in the streaming path; the sequential path typically logs and exits on failure instead of returning `Err`.
pub fn run(
    dir_to_ublx: &Path,
    ublx_opts: &UblxOpts,
    prior_nefax: Option<&integrations::NefaxResult>,
) -> io::Result<(usize, usize, usize)> {
    let db_path = UblxPaths::new(dir_to_ublx).db();
    // No real indexed paths yet but a leftover `.nefaxer` lets nefax match disk → empty diff (0/0/0) and a stuck first-run TUI.
    if !db_ops::snapshot_has_indexed_paths(&db_path) {
        let _ = db_ops::UblxCleanup::delete_nefaxer_files(dir_to_ublx);
    }
    let mode = RunMode::from_opts(ublx_opts);
    match mode {
        RunMode::Sequential => run_sequential(dir_to_ublx, ublx_opts, prior_nefax),
        RunMode::Stream => run_stream(dir_to_ublx, ublx_opts, prior_nefax),
    }
}

/// Run nefax, then batch zahir on paths that need it, then write the snapshot DB.
///
/// # Errors
///
/// Returns [`std::io::Error`] only if a propagated I/O error occurs; most failures log and exit the process instead.
pub fn run_sequential(
    dir_to_ublx: &Path,
    ublx_opts: &UblxOpts,
    prior_nefax: Option<&integrations::NefaxResult>,
) -> io::Result<(usize, usize, usize)> {
    let (dir_to_ublx_abs, prior_zahir_json, prior_category) = pre_run_setup(dir_to_ublx);
    let (nefax, diff) = run_nefax_exiting::<fn(&integrations::NefaxEntry)>(
        dir_to_ublx,
        ublx_opts,
        prior_nefax,
        None,
    );
    let nefax = integrations::nefax_collapse_zarr_inners(nefax);
    let diff = integrations::nefax_diff_collapse_zarr_inners(diff);

    debug!(
        "indexed {} paths (added: {}, removed: {}, modified: {})",
        nefax.len(),
        diff.added.len(),
        diff.removed.len(),
        diff.modified.len()
    );
    let force_full = should_force_full_zahir(ublx_opts);
    let path_list = paths_needing_zahir(
        &nefax,
        prior_nefax,
        &dir_to_ublx_abs,
        ublx_opts,
        force_full,
        &prior_zahir_json,
    );

    debug!(
        "zahir running on {} paths (force_full={force_full})",
        path_list.len()
    );

    let zahir_result = match integrations::run_zahir_batch(&path_list, ublx_opts) {
        Ok(r) => r,
        Err(e) => {
            error!("{}", utils::NefaxZahirErrors::zahir_sequential_failed(&e));
            utils::exit_error();
        }
    };
    utils::write_zahir_failures_to_log_error(dir_to_ublx, &zahir_result);
    let batch_zahir = batch_zahir_predicate(ublx_opts);
    let prior_ctx = snapshot_prior_ctx(&prior_zahir_json, &prior_category, &batch_zahir);
    if let Err(e) = db_ops::write_snapshot_to_db(
        dir_to_ublx,
        &nefax,
        Some(&zahir_result),
        &diff,
        &ublx_opts.to_ublx_settings(),
        &prior_ctx,
    ) {
        error!("failed to write snapshot: {e}");
        utils::exit_error();
    }
    Ok((diff.added.len(), diff.modified.len(), diff.removed.len()))
}

/// Run nefax with streaming zahir output, then write the snapshot DB from the streamed channel.
///
/// # Errors
///
/// Returns [`std::io::Error`] only if a propagated I/O error occurs; most failures log and exit the process instead.
pub fn run_stream(
    dir_to_ublx: &Path,
    ublx_opts: &UblxOpts,
    prior_nefax: Option<&integrations::NefaxResult>,
) -> io::Result<(usize, usize, usize)> {
    let (dir_to_ublx_abs, prior_zahir_json, prior_category) = pre_run_setup(dir_to_ublx);

    let ublx_opts_for_zahir = ublx_opts.clone();
    let force_full = should_force_full_zahir(ublx_opts);
    let (path_tx, path_rx) = mpsc::channel();
    let (output_tx, output_rx) = mpsc::channel();
    let zahir_handle = std::thread::spawn(move || {
        let output_sink = integrations::ZahirOutputSink::Channel(output_tx);
        integrations::run_zahir_from_stream(&path_rx, &ublx_opts_for_zahir, &output_sink)
    });
    let batch_zahir_ctx = BatchZahirCtx {
        dir_to_ublx_abs: dir_to_ublx_abs.as_path(),
        ublx_opts,
        force_full_zahir: force_full,
        prior_nefax,
        prior_zahir_json: &prior_zahir_json,
    };
    let zahir_stream_seen = std::sync::Mutex::new(HashSet::new());
    let on_entry = |e: &integrations::NefaxEntry| {
        if !entry_should_batch_zahir(&batch_zahir_ctx, &e.path, e.size, e.mtime_ns) {
            return;
        }
        let full = dir_to_ublx_abs.join(&e.path);
        let full = integrations::zarr_collapse_to_store_root_path_or_same(&full);
        let s = full.to_string_lossy().into_owned();
        if let Ok(mut g) = zahir_stream_seen.lock()
            && g.insert(s.clone())
        {
            let _ = path_tx.send(s);
        }
    };
    let (nefax, diff) =
        match integrations::run_nefaxer(dir_to_ublx, ublx_opts, prior_nefax, Some(on_entry)) {
            Ok(result) => result,
            Err(e) => {
                drop(path_tx);
                let _ = zahir_handle.join();
                on_nefax_error(dir_to_ublx, &e);
            }
        };
    let nefax = integrations::nefax_collapse_zarr_inners(nefax);
    let diff = integrations::nefax_diff_collapse_zarr_inners(diff);

    drop(path_tx);
    debug!("indexed {} paths (streaming)", nefax.len());
    let batch_zahir = batch_zahir_predicate(ublx_opts);
    let prior_ctx = snapshot_prior_ctx(&prior_zahir_json, &prior_category, &batch_zahir);
    if let Err(e) = db_ops::write_snapshot_to_db_streaming(
        dir_to_ublx,
        &nefax,
        &diff,
        &ublx_opts.to_ublx_settings(),
        &output_rx,
        &prior_ctx,
    ) {
        error!("failed to write snapshot: {e}");
        utils::exit_error();
    }

    match zahir_handle.join() {
        Ok(Ok(r)) => {
            utils::write_zahir_failures_to_log_error(dir_to_ublx, &r);
        }
        Ok(Err(e)) => {
            error!("{}", utils::NefaxZahirErrors::zahir_stream_failed(&e));
            utils::exit_error();
        }
        Err(_) => {
            error!("{}", utils::NefaxZahirErrors::ZAHIR_THREAD_PANICKED);
            utils::exit_error();
        }
    }
    Ok((diff.added.len(), diff.modified.len(), diff.removed.len()))
}
