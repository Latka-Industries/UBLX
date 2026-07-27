//! Zarr store right pane: `tree` output as the viewer body plus zahir-derived templates / metadata / writing.

use std::path::Path;
use std::sync::Arc;

use crate::config::UblxOpts;
use crate::layout::setup::{RightPaneContent, RightPaneContentDerived, SnapshotEntryMeta};

use super::core::{
    directory_tree_policy_line, tree_subprocess_for_path, zahir_derived_pane_fields,
    zahir_json_string_for_path,
};

/// Row + options for a Zarr store right pane (tree body + zahir templates / metadata / writing).
pub struct ZarrStoreRightPaneView<'a> {
    pub path: &'a str,
    pub category: &'a str,
    pub size: u64,
    pub full_path: std::path::PathBuf,
    pub viewer_mtime_ns: Option<i64>,
    pub enable_enhance_all: bool,
    pub ublx_opts: &'a UblxOpts,
}

/// [`ZarrStoreRightPaneView`] plus DB path to load `zahir_json` in [`build_zarr_store_right_pane`].
pub struct ZarrStoreRightPaneBuild<'a> {
    pub db_path: &'a Path,
    pub path: &'a str,
    pub category: &'a str,
    pub size: u64,
    pub full_path: &'a Path,
    pub viewer_mtime_ns: Option<i64>,
    pub enable_enhance_all: bool,
    pub ublx_opts: &'a UblxOpts,
}

impl<'a> ZarrStoreRightPaneBuild<'a> {
    #[must_use]
    fn to_view(&self) -> ZarrStoreRightPaneView<'a> {
        ZarrStoreRightPaneView {
            path: self.path,
            category: self.category,
            size: self.size,
            full_path: self.full_path.to_path_buf(),
            viewer_mtime_ns: self.viewer_mtime_ns,
            enable_enhance_all: self.enable_enhance_all,
            ublx_opts: self.ublx_opts,
        }
    }
}

/// `tree` stdout or the same user-visible error string as the viewer (Ok/Err unifies to one body).
fn tree_subprocess_unified_for_viewer(full_path: &Path) -> String {
    match tree_subprocess_for_path(full_path) {
        Ok(s) => s,
        Err(e) => e,
    }
}

/// Zarr store directory: `tree` + DB + zahir (for [`super::async_ops::drive_right_pane_async`], off the UI thread).
#[must_use]
pub fn build_zarr_store_right_pane(b: &ZarrStoreRightPaneBuild<'_>) -> RightPaneContent {
    let tree_str = tree_subprocess_unified_for_viewer(b.full_path);
    let zahir_json = zahir_json_string_for_path(b.db_path, b.path);
    let view = b.to_view();
    right_pane_from_zarr_tree_and_zahir(tree_str, &zahir_json, &view)
}

/// Build [`RightPaneContent`] from a tree string and zahir JSON (sync path uses a cached `tree` run).
pub(crate) fn right_pane_from_zarr_tree_and_zahir(
    tree_str: String,
    zahir_json: &str,
    view: &ZarrStoreRightPaneView<'_>,
) -> RightPaneContent {
    let (templates, template_views, metadata, writing, offer_zahir) =
        zahir_derived_pane_fields(zahir_json, view.enable_enhance_all);
    let policy_line = Some(directory_tree_policy_line(view.ublx_opts, view.path));
    RightPaneContent {
        templates,
        template_views,
        metadata,
        writing,
        viewer: Some(Arc::from(tree_str)),
        viewer_directory_policy_line: policy_line,
        snap_meta: SnapshotEntryMeta {
            path: Some(view.path.to_string()),
            category: Some(view.category.to_string()),
            size: Some(view.size),
            mtime_ns: view.viewer_mtime_ns,
            has_zahir_json: !zahir_json.is_empty(),
        },
        derived: RightPaneContentDerived {
            abs_path: Some(view.full_path.clone()),
            can_open: false,
            offer_enhance_zahir: offer_zahir,
            offer_enhance_directory_policy: false,
            embedded_cover_raster: None,
        },
    }
}
