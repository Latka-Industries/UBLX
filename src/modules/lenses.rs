//! Lens DB helpers: create/select lenses, add or remove paths. Popup and key handling live in
//! [`crate::ui::menus::lens_handler`].

use std::path::Path;

use crate::engine::db_ops;

/// Add the given path to a lens by name at the end of the list. No-op if lens missing.
/// Caller should refresh `lens_paths` / view when in Lenses mode.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors from [`crate::engine::db_ops`].
pub fn add_path_to_lens(db_path: &Path, lens_name: &str, path: &str) -> Result<(), anyhow::Error> {
    let count =
        i64::try_from(db_ops::load_lens_paths(db_path, lens_name)?.len()).unwrap_or(i64::MAX);
    db_ops::add_path_to_lens(db_path, lens_name, path, count)
}

/// Create a new lens by name. Errors if name already exists.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors (e.g. duplicate lens name).
pub fn create_lens(db_path: &Path, name: &str) -> Result<i64, anyhow::Error> {
    db_ops::create_lens(db_path, name)
}

/// Remove a path from a lens. No-op if path not in lens.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors from [`crate::engine::db_ops`].
pub fn remove_path_from_lens(
    db_path: &Path,
    lens_name: &str,
    path: &str,
) -> Result<(), anyhow::Error> {
    db_ops::remove_path_from_lens(db_path, lens_name, path)
}

/// Rename a lens. Errors if new name is empty or already exists.
///
/// # Errors
///
/// Returns [`anyhow::Error`] when validation or `SQLite` operations fail.
pub fn rename_lens(db_path: &Path, old_name: &str, new_name: &str) -> Result<(), anyhow::Error> {
    db_ops::rename_lens(db_path, old_name, new_name)
}

/// Delete a lens and all its path associations.
///
/// # Errors
///
/// Returns [`anyhow::Error`] on `SQLite` errors from [`crate::engine::db_ops`].
pub fn delete_lens(db_path: &Path, lens_name: &str) -> Result<(), anyhow::Error> {
    db_ops::delete_lens(db_path, lens_name)
}
