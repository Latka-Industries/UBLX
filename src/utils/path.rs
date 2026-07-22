//! Path helpers (extensions, etc.).

use std::fs;
use std::path::{Path, PathBuf};

/// Resolve a path string from the DB or snapshot against `base` when relative, or use it as-is when absolute.
///
/// Same behavior as [`Path::join`]: if `path` is absolute, it replaces the prefix under `base`.
#[must_use]
pub fn resolve_under_root(base: &Path, path: &str) -> PathBuf {
    base.join(path)
}

/// True if `path` relative to `root` exists on disk and is a directory (`fs::metadata` / `is_dir`).
/// Matches how snapshot rows get category `"Directory"` (see `db_ops` category fallback).
#[must_use]
pub fn rel_path_is_directory(root: &Path, path: &Path) -> bool {
    fs::metadata(root.join(path)).is_ok_and(|m| m.is_dir())
}

/// Path as a string with `/` separators (TOML paths, policy prefix checks, DB keys, cross-platform snapshot maps).
///
/// On Windows, normalizes `\\` to `/` so comparisons match Unix-style config and stored strings.
#[must_use]
pub fn path_to_slash_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Normalize a snapshot `path` column so it matches nefaxer’s relative path strings (`rel_str` / map keys).
///
/// Trims, strips a leading `./` or `.\`, then replaces `\` with `/`.
#[must_use]
pub fn normalize_snapshot_rel_path_str(path: &str) -> String {
    let mut s = path.trim();
    s = s.strip_prefix("./").unwrap_or(s);
    if let Some(rest) = s.strip_prefix(".\\") {
        s = rest;
    }
    s.replace('\\', "/")
}

/// [`PathBuf`] key for nefax-style maps, from a snapshot `path` column (see [`normalize_snapshot_rel_path_str`]).
#[must_use]
pub fn snapshot_rel_path_buf(path_str: &str) -> PathBuf {
    PathBuf::from(normalize_snapshot_rel_path_str(path_str))
}

/// Collapse long slash-separated paths for compact UI titles.
///
/// If there are more than [`TAIL`] non-empty components (after normalizing `\\` to `/`), returns
/// `.../x/y/z` using only the last [`TAIL`] segments. Otherwise returns the trimmed path with `\`
/// normalized to `/`.
#[must_use]
pub fn shorten_path_for_title(path: &str) -> String {
    const TAIL: usize = 3;
    let normalized = path.trim().replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= TAIL {
        return normalized;
    }
    let tail = parts[parts.len() - TAIL..].join("/");
    format!(".../{tail}")
}

/// True if `path`'s file extension equals any of `exts` (ASCII case-insensitive, OR semantics).
#[must_use]
pub fn path_has_extension(path: &str, exts: &[&str]) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| exts.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

/// Define a `fn name(path: &str) -> bool` that checks the path suffix against a fixed extension list.
///
/// # Example
///
/// ```ignore
/// define_path_ext_predicate! {
///     #[must_use]
///     pub fn is_markdown_path(path: &str) -> bool {
///         "md", "markdown"
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_path_ext_predicate {
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($path:ident: &str) -> bool {
            $($ext:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis fn $name(path: &str) -> bool {
            $crate::utils::path_has_extension(path, &[$($ext),+])
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{path_has_extension, resolve_under_root, shorten_path_for_title};
    use std::path::{Path, PathBuf};

    #[test]
    fn shorten_path_for_title_short_unchanged() {
        assert_eq!(shorten_path_for_title("a/b"), "a/b");
        assert_eq!(shorten_path_for_title("a/b/c"), "a/b/c");
        assert_eq!(shorten_path_for_title("file.rs"), "file.rs");
    }

    #[test]
    fn shorten_path_for_title_long_keeps_last_three() {
        assert_eq!(shorten_path_for_title("a/b/c/d"), ".../b/c/d");
        assert_eq!(
            shorten_path_for_title("/usr/local/bin/tool"),
            ".../local/bin/tool"
        );
    }

    #[test]
    fn shorten_path_for_title_normalizes_backslashes() {
        assert_eq!(shorten_path_for_title(r"a\b\c\d"), ".../b/c/d");
    }

    #[test]
    fn path_has_extension_matches_final_segment() {
        assert!(path_has_extension("foo.md", &["md"]));
        assert!(path_has_extension("foo.MD", &["md"]));
        assert!(path_has_extension("a/b/c.markdown", &["markdown"]));
        assert!(path_has_extension("a/b/c.MARKDOWN", &["markdown"]));
    }

    #[test]
    fn path_has_extension_rejects_non_matching() {
        assert!(!path_has_extension("foo.txt", &["md"]));
        assert!(!path_has_extension("foo", &["md"]));
        assert!(!path_has_extension("foo.md.bak", &["md"]));
    }

    #[test]
    fn resolve_under_root_joins_relative() {
        let base = Path::new("project");
        assert_eq!(
            resolve_under_root(base, "a/b"),
            PathBuf::from("project").join("a/b")
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolve_under_root_absolute_replaces_prefix() {
        assert_eq!(
            resolve_under_root(Path::new("/proj/.ublx"), "/x/y"),
            PathBuf::from("/x/y")
        );
    }
}
