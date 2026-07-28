//! Catalog paths, SQLite schema/ops, and headless open/read (THI-155 Phase 2).
//!
//! Layers, bottom up:
//! - [`paths`]: names, user dirs, and [`UblxPaths`] for an indexed root.
//! - [`db_ops`]: `.ublx` schema plus snapshot / settings / `delta_log` / lens reads and writes.
//! - [`open`] / [`read`]: resolve a directory to a read connection and run shared list/detail queries.
//!
//! Nothing here depends on the TUI, `UblxOpts`, or the `ublx` `serve` feature: `nefaxer` / `zahirscan`
//! types are used directly ([`nefax`], [`zahir`]) and callers pass plain data ([`UblxSettings`]) or
//! closures instead of app options.

pub mod db_ops;
pub mod nefax;
pub mod open;
pub mod paths;
pub mod read;
pub mod settings;
pub mod util;
pub mod zahir;

pub use nefax::{NefaxDiff, NefaxPathMeta, NefaxResult};
pub use paths::{
    UBLX_NAMES, UblxNames, UblxPaths, cache_dir, config_dir, db_dir, get_log_path,
    global_config_toml, hash_suffix_from_db_stem, is_hex_hash16, last_applied_config_path,
    normalize_rel_path_for_policy, path_is_under_or_equal, path_to_hex,
    rel_path_is_exact_local_config_toml,
};
pub use settings::UblxSettings;
pub use util::{
    canonicalize_dir_to_ublx, expand_home_dir_arg, get_created_ns, normalize_snapshot_rel_path_str,
    snapshot_rel_path_buf, try_validate_dir,
};
pub use zahir::{
    ZahirFT, ZahirOutput, ZahirResult, file_type_from_metadata_name, get_zahir_output_by_path,
    zahir_metadata_name_from_indexed_file, zahir_metadata_name_from_path_hint,
    zahir_output_to_json, zahir_output_to_json_for_path,
};
