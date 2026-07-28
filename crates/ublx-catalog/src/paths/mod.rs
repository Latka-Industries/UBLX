//! On-disk paths for indexed roots: names, user dirs, and [`UblxPaths`].

mod dirs;
mod names;
mod ublx_paths;

pub use dirs::{
    cache_dir, config_dir, db_dir, global_config_toml, last_applied_config_path,
    rel_path_is_exact_local_config_toml,
};
pub use names::{UBLX_NAMES, UblxNames, hash_suffix_from_db_stem, is_hex_hash16, path_to_hex};
pub use ublx_paths::{
    UblxPaths, get_log_path, normalize_rel_path_for_policy, path_is_under_or_equal,
};

#[cfg(test)]
mod tests {
    use super::{UBLX_NAMES, hash_suffix_from_db_stem, is_hex_hash16};

    #[test]
    fn index_db_file_ext_is_dot_ublx() {
        assert_eq!(UBLX_NAMES.index_db_file_ext, ".ublx");
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
