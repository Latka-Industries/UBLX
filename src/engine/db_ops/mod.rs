//! `SQLite` **`.ublx`** database (per ublx root / working tree): schema DDL and statement constants, open/create,
//! and read/write of **snapshot** rows (paths, mtimes, hashes, `category`, `zahir_json`), **settings**,
//! **`delta_log`** (per-index run: added / modified / removed), **lens** / **`lens_path`**, and related
//! helpers (live snapshot wiring, path resolution, duplicate extraction, zahir export).
//!
//! The crate-facing API is the **re-exports** from this module (for example [`UblxDbCategory`],
//! [`UblxDbSchema`], [`SnapshotTuiRow`], and zahir/snapshot load helpers).

mod consts;
mod core;
mod delta_log;
mod extract_duplicates;
mod lens_export;
mod lens_storage;
mod live_snapshot;
mod path_resolver;
mod utils;
mod zahir_export;

pub use consts::*;
pub use core::*;
pub use delta_log::*;
pub use extract_duplicates::*;
pub use lens_export::*;
pub use lens_storage::*;
pub use live_snapshot::*;
pub use path_resolver::*;
pub use utils::*;
pub use zahir_export::*;

#[cfg(test)]
mod tests;
