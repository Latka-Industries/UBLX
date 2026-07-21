//! Headless CLI subcommands (`query`, `doctor`, `serve`) and shared catalog resolve/open.
//!
//! Keep this module free of ratatui / TUI dependencies so catalog commands stay lean.

mod catalog;
mod catalog_read;
mod doctor;
mod output;
mod query;
mod serve;

pub use catalog::{
    CatalogHandle, CatalogPaths, open_catalog_for_read, resolve_catalog_paths,
    snapshot_likely_in_progress,
};
pub use catalog_read::{CatalogNotFound, DeltaRow, EntryListFilter, EntryRow};

use crate::cli_parser::Commands;

/// Dispatch a parsed subcommand. Returns after the handler finishes (or errors).
///
/// # Errors
///
/// Propagates handler errors (missing DB, invalid DIR, `SQLite` failures).
pub fn run(command: &Commands) -> Result<(), anyhow::Error> {
    match command {
        Commands::Query(args) => query::run(args),
        Commands::Doctor(args) => doctor::run(args),
        Commands::Serve(args) => serve::run(args),
    }
}
