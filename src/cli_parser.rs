use std::path::PathBuf;

use clap::{Parser, Subcommand};
use log::debug;

use crate::themes;
use crate::utils;

#[derive(Parser)]
#[command(
    name = "ublx",
    version,
    about = "UBLX is a TUI to index once, enrich with metadata, and browse a flat snapshot in a 3-pane layout with multiple modes."
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,
    /// Directory to index (when no subcommand)
    #[arg(value_name = "DIR", default_value = ".")]
    pub dir_to_ublx: PathBuf,
    #[command(flatten)]
    pub headless: HeadlessCli,
    /// Dev mode: tui-logger drain + `move_events` + trace-level default filter
    #[arg(long = "dev")]
    pub dev: bool,
    /// Print available themes grouped by appearance
    #[arg(long = "themes")]
    pub themes: bool,
}

/// Headless catalog subcommands (`query`, `doctor`, `serve`).
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Query the `.ublx` catalog (list / detail / delta / lenses)
    Query(QueryCli),
    /// Diagnose `.ublx` DB / path / schema
    Doctor(DoctorCli),
    /// Serve a local read-only HTTP API over the `.ublx` catalog
    Serve(ServeCli),
}

/// `ublx serve [DIR]` — JSON API via panza (`--host` / `--port` / `--open`).
#[derive(Parser, Debug)]
pub struct ServeCli {
    /// Indexed directory whose catalog to serve
    #[arg(value_name = "DIR", default_value = ".")]
    pub dir: PathBuf,
    #[command(flatten)]
    pub serve: panza::ServeArgs,
}

/// Shared `--url` / `UBLX_URL` for headless commands that can talk to `ublx serve`.
#[derive(Parser, Debug, Default)]
pub struct RemoteCli {
    /// Base URL of a running `ublx serve` (env: `UBLX_URL`). When set, local `DIR` is ignored.
    #[arg(long, env = "UBLX_URL")]
    pub url: Option<String>,
}

/// `ublx query [DIR]`
#[derive(Parser, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct QueryCli {
    /// Indexed directory whose catalog to query (ignored when `--url` / `UBLX_URL` is set)
    #[arg(value_name = "DIR", default_value = ".")]
    pub dir: PathBuf,
    #[command(flatten)]
    pub remote: RemoteCli,
    /// Emit JSON instead of a human table
    #[arg(long)]
    pub json: bool,
    /// Filter snapshot rows by exact category name
    #[arg(long)]
    pub category: Option<String>,
    /// Keep rows with size >= N bytes
    #[arg(long = "min-size")]
    pub min_size: Option<u64>,
    /// Keep rows with size <= N bytes
    #[arg(long = "max-size")]
    pub max_size: Option<u64>,
    /// Keep rows whose path contains this substring
    #[arg(long)]
    pub contains: Option<String>,
    /// Show one snapshot row by exact relative path
    #[arg(long)]
    pub path: Option<String>,
    /// With `--path`, include `zahir_json`
    #[arg(long)]
    pub zahir: bool,
    /// List distinct snapshot categories
    #[arg(long)]
    pub categories: bool,
    /// List lens names
    #[arg(long)]
    pub lenses: bool,
    /// List paths in a named lens
    #[arg(long)]
    pub lens: Option<String>,
    /// List `delta_log` rows (newest first)
    #[arg(long)]
    pub delta: bool,
    /// With `--delta`, filter by type: `added`, `mod`, or `removed`
    #[arg(long = "delta-type")]
    pub delta_type: Option<String>,
}

/// `ublx doctor [DIR]`
#[derive(Parser, Debug)]
pub struct DoctorCli {
    /// Indexed directory whose catalog to diagnose (ignored when `--url` / `UBLX_URL` is set)
    #[arg(value_name = "DIR", default_value = ".")]
    pub dir: PathBuf,
    #[command(flatten)]
    pub remote: RemoteCli,
    /// Emit machine-readable JSON instead of human text
    #[arg(long)]
    pub json: bool,
    /// Remove leftover tmp / wal / shm aux files (not the main `.ublx` DB)
    #[arg(long)]
    pub fix: bool,
    /// Run even if a snapshot appears in progress (tmp + wal/shm present)
    #[arg(long)]
    pub force: bool,
}

/// Headless indexing flag
#[derive(Parser)]
#[allow(clippy::struct_excessive_bools)]
pub struct HeadlessCli {
    /// Headless snapshot. Writes a local config file when this dir has none.
    #[arg(long = "snapshot-only", short = 's')]
    pub snapshot_only: bool,
    /// With `--snapshot-only`: set `enable_enhance_all = true` in new local config and use it for this run.
    #[arg(long = "enhance-all", short = 'e')]
    pub enhance_all: bool,
    /// Same as `--snapshot-only --enhance-all`.
    #[arg(long = "full-snapshot", short = 'f')]
    pub full_snapshot: bool,
    /// Headless: write each Zahir JSON to `ublx-export/` as flat `{path}.json` files. Recommended to run with "--full-snapshot" to get most complete & recent results. Adjust enhance policy in config to fine-tune which paths get `ZahirScan`.
    #[arg(long = "export", short = 'x')]
    pub export_zahir: bool,
}

pub fn print_available_themes() {
    for entry in themes::theme_selector_entries() {
        match entry {
            themes::SelectorEntry::Section(label) => {
                println!("{label}:");
            }
            themes::SelectorEntry::Item(theme) => {
                println!("  - {}", theme.name);
            }
        }
    }
}

/// Headless snapshot flavor: `-s` (optionally with `-e`) or `-f` (implies enhance-all for the run).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotType {
    /// `--snapshot-only`; `enhance_all` reflects `--enhance-all`.
    MinSnapshot { enhance_all: bool },
    /// `--full-snapshot` (same as `--snapshot-only --enhance-all` for this run).
    FullSnapshot,
}

/// Normalized headless CLI: optional snapshot pass, optional export pass (both may be set → snapshot then export).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadlessModeFlags {
    /// `None` when neither `-s` nor `-f`.
    pub snapshot: Option<SnapshotType>,
    /// `--export` / `-x`.
    pub export: bool,
}

impl HeadlessModeFlags {
    #[must_use]
    pub fn new(args_headless: &HeadlessCli) -> Self {
        let snapshot = if args_headless.full_snapshot {
            Some(SnapshotType::FullSnapshot)
        } else if args_headless.snapshot_only {
            Some(SnapshotType::MinSnapshot {
                enhance_all: args_headless.enhance_all,
            })
        } else {
            None
        };
        Self {
            snapshot,
            export: args_headless.export_zahir,
        }
    }

    /// True when any headless work runs (no TUI): snapshot pass and/or export.
    #[must_use]
    pub fn is_headless(self) -> bool {
        self.snapshot.is_some() || self.export
    }

    /// Whether this headless snapshot run should enable enhance-all (`-f`, or `-s -e`).
    #[must_use]
    pub fn determine_enhance_all(self) -> bool {
        match self.snapshot {
            Some(SnapshotType::FullSnapshot) => true,
            Some(SnapshotType::MinSnapshot { enhance_all }) => enhance_all,
            None => false,
        }
    }
}

#[must_use]
pub fn headless_handler(args_headless: &HeadlessCli) -> HeadlessModeFlags {
    let flags = HeadlessModeFlags::new(args_headless);
    if args_headless.full_snapshot && args_headless.enhance_all {
        debug!("Full snapshot with --enhance-all is redundant; use --full-snapshot (-f) alone.");
    }
    utils::exit_if_enhance_all_without_headless(
        args_headless.enhance_all,
        flags.snapshot.is_some(),
    );
    flags
}
