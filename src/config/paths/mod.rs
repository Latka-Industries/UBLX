//! On-disk paths for indexed roots: names, user dirs, recents, and [`UblxPaths`].
//!
//! Core resolve (`names` / `dirs` / `UblxPaths`) lives in `ublx-catalog`; this module keeps
//! welcome/recents scoring in the binary crate and re-exports the catalog surface.

mod recents;

pub use recents::{
    all_indexed_roots_alphabetical, has_any_cached_ublx_db, has_recents_entry_for_dir,
    prior_indexed_roots, prior_indexed_roots_recent, record_prior_root_selected,
    record_ublx_session_open, remember_indexed_root_path, should_show_initial_prompt,
};
pub use ublx_catalog::paths::{
    UBLX_NAMES, UblxNames, UblxPaths, cache_dir, config_dir, db_dir, get_log_path,
    global_config_toml, hash_suffix_from_db_stem, is_hex_hash16, last_applied_config_path,
    normalize_rel_path_for_policy, path_is_under_or_equal, path_to_hex,
    rel_path_is_exact_local_config_toml,
};

#[cfg(test)]
mod tests {
    use super::{UBLX_NAMES, hash_suffix_from_db_stem, is_hex_hash16, should_show_initial_prompt};

    #[test]
    fn never_in_snapshot_only_mode() {
        assert!(!should_show_initial_prompt(true, false, false));
        assert!(!should_show_initial_prompt(true, true, false));
    }

    #[test]
    fn initial_prompt_only_if_no_ubli_db_when_not_snapshot_only() {
        assert!(should_show_initial_prompt(false, false, false));
        assert!(!should_show_initial_prompt(false, true, false));
    }

    #[test]
    fn index_db_file_ext_is_dot_pkg_name() {
        assert_eq!(
            UBLX_NAMES.index_db_file_ext,
            format!(".{}", UBLX_NAMES.pkg_name)
        );
        assert_eq!(UBLX_NAMES.pkg_name, "ublx");
    }

    #[test]
    fn hex_hash16_accepts_16_hex_digits() {
        assert!(is_hex_hash16("729a9c26db109730"));
        assert!(!is_hex_hash16("729a9c26db10973"));
        assert!(!is_hex_hash16("729a9c26db1097300"));
        assert!(!is_hex_hash16("g29a9c26db109730"));
    }

    #[test]
    fn hash_suffix_from_db_stem_parses_stem() {
        assert_eq!(
            hash_suffix_from_db_stem("mydir_729a9c26db109730"),
            Some("729a9c26db109730")
        );
        assert_eq!(hash_suffix_from_db_stem("729a9c26db109730"), None);
        assert_eq!(hash_suffix_from_db_stem("no_hash_here"), None);
    }
}
