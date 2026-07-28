use nefaxer::{Nefax, Result, nefax_dir, tuning_for_path};
use std::path::Path;

use log::{debug, error};

use crate::config::{UblxOpts, UblxSettings, parse_drive_type};

pub type NefaxOpts = nefaxer::NefaxOpts;
pub type NefaxEntry = nefaxer::Entry;
pub type NefaxDriveType = nefaxer::disk_detect::DriveType;

/// Snapshot-facing aliases are owned by `ublx-catalog` (it reads/writes the `snapshot` table).
pub use ublx_catalog::{NefaxDiff, NefaxPathMeta, NefaxResult};

fn nefax_opts_with_tuning(
    exclude: &[String],
    num_threads: usize,
    drive_type: NefaxDriveType,
    use_parallel_walk: bool,
) -> NefaxOpts {
    let mut opts = NefaxOpts {
        num_threads: Some(num_threads),
        drive_type: Some(drive_type),
        use_parallel_walk: Some(use_parallel_walk),
        ..NefaxOpts::default()
    };
    opts.exclude.extend(exclude.iter().map(ToString::to_string));
    opts
}

/// Build `NefaxOpts` for indexing `dir` with `exclude`. When `cached_settings` is `Some`, use those values and skip disk check; otherwise call [`tuning_for_path`](nefaxer::tuning_for_path).
#[must_use]
pub fn pre_opts_for_nefaxer(
    dir_to_ublx: &Path,
    exclude: &[String],
    cached_settings: Option<&UblxSettings>,
) -> NefaxOpts {
    let (num_threads, drive_type, use_parallel_walk) = match cached_settings {
        Some(s) => (
            s.num_threads,
            parse_drive_type(&s.drive_type),
            s.parallel_walk,
        ),
        None => tuning_for_path(dir_to_ublx, None),
    };
    nefax_opts_with_tuning(exclude, num_threads, drive_type, use_parallel_walk)
}

fn extract_nefax_opts_from_ublx_opts(opts: &UblxOpts) -> NefaxOpts {
    opts.nefax_opts_with_workers()
}

/// Run nefaxer; on success return `(nefax, diff)`, on error log and exit. Use when no cleanup is needed (e.g. sequential).
///
/// # Errors
///
/// Returns [`anyhow::Error`] when indexing fails (I/O, walk errors, etc.).
pub fn run_nefaxer<F>(
    dir_to_ublx: &Path,
    ublx_opts: &UblxOpts,
    nefax: Option<&Nefax>,
    entry_callback: Option<F>,
) -> Result<(NefaxResult, NefaxDiff)>
where
    F: FnMut(&NefaxEntry),
{
    let nefax_opts = extract_nefax_opts_from_ublx_opts(ublx_opts);
    nefax_dir(dir_to_ublx, &nefax_opts, nefax, entry_callback)
}

/// Load prior Nefax at startup. Exits the process on DB error; returns `None` when no prior snapshot exists.
#[must_use]
pub fn load_prior_nefax_or_exit(dir_to_ublx: &Path, db_path: &Path) -> Option<NefaxResult> {
    match ublx_catalog::db_ops::load_nefax_from_db(dir_to_ublx, db_path) {
        Ok(Some(nefax)) => {
            debug!("loaded {} paths from snapshot", nefax.len());
            Some(nefax)
        }
        Ok(None) => None,
        Err(e) => {
            error!("failed to load snapshot: {e}");
            crate::utils::exit_error();
        }
    }
}
