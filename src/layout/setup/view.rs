//! Right-pane content and derived list/delta view data.

use std::path::PathBuf;
use std::sync::Arc;

use crate::engine::db_ops::{DeltaType, UblxDbCategory};
use crate::integrations::{ZahirFT, file_type_from_metadata_name};

use super::TuiRow;

/// Avoids re-reading the selected file every UI tick when path, category, size, and mtime match.
#[derive(Debug, Clone)]
pub struct ViewerDiskContentCache {
    pub rel_path: String,
    /// Snapshot category (drives file-type handling in the viewer).
    pub category: String,
    pub file_len: u64,
    pub modified: Option<std::time::SystemTime>,
    pub viewer_str: Option<String>,
    pub embedded_cover_raster: Option<Vec<u8>>,
    pub viewer_can_open: bool,
}

impl ViewerDiskContentCache {
    #[must_use]
    pub fn matches(&self, path: &str, category: &str, meta: &std::fs::Metadata) -> bool {
        self.rel_path == path
            && self.category == category
            && self.file_len == meta.len()
            && self.modified == meta.modified().ok()
    }
}

#[derive(Default)]
pub struct RightPaneAsync {
    pub generation: u64,
    pub last_spawn_path: String,
    pub displayed: RightPaneContent,
    pub rx: Option<tokio::sync::mpsc::UnboundedReceiver<RightPaneAsyncReady>>,
}

/// Per-pane content from zahir JSON. Templates always present; metadata and writing only if keys exist.
pub struct SectionedPreview {
    pub templates: String,
    pub metadata: Option<String>,
    pub writing: Option<String>,
}

/// Snapshot mode: indices into the single in-memory list (no copy). Delta mode: small owned vec.
#[derive(Clone)]
pub enum ViewContents {
    /// Indices into the caller's `all_rows` slice (snapshot mode — one copy of list).
    SnapshotIndices(Vec<usize>),
    /// Owned rows for delta mode (added/mod/removed paths; typically small).
    DeltaRows(Vec<TuiRow>),
}

/// Derived list data for this tick: filtered categories and contents (by index or owned), lengths for navigation.
/// Scalability: snapshot mode uses [`ViewContents::SnapshotIndices`] so we keep a single copy of the list; no cloned row vec.
pub struct ViewData {
    pub filtered_categories: Vec<String>,
    pub contents: ViewContents,
    pub category_list_len: usize,
    pub content_len: usize,
}

impl ViewData {
    /// Row at content index `i`. For [`ViewContents::SnapshotIndices`], pass `Some(all_rows)`; for [`ViewContents::DeltaRows`], pass `None`.
    #[must_use]
    pub fn row_at<'a>(&'a self, i: usize, all_rows: Option<&'a [TuiRow]>) -> Option<&'a TuiRow> {
        match &self.contents {
            ViewContents::SnapshotIndices(indices) => indices
                .get(i)
                .and_then(|&pos| all_rows.and_then(|r| r.get(pos))),
            ViewContents::DeltaRows(rows) => rows.get(i),
        }
    }

    /// Iterate over content rows. For [`ViewContents::SnapshotIndices`], pass `Some(all_rows)`; for [`ViewContents::DeltaRows`], pass `None`.
    #[must_use]
    pub fn iter_contents<'a>(
        &'a self,
        all_rows: Option<&'a [TuiRow]>,
    ) -> Box<dyn Iterator<Item = &'a TuiRow> + 'a> {
        match &self.contents {
            ViewContents::SnapshotIndices(indices) => {
                let iter = indices
                    .iter()
                    .filter_map(move |&pos| all_rows.and_then(|r| r.get(pos)));
                Box::new(iter)
            }
            ViewContents::DeltaRows(rows) => Box::new(rows.iter()),
        }
    }
}

/// Raw delta row: (`created_ns`, path) from `delta_log`. Used to build display lines with dates preserved when filtering.
pub type DeltaRow = (i64, String);

/// Data for Delta mode: snapshot overview text and raw (`created_ns`, path) rows per delta type.
pub struct DeltaViewData {
    pub overview_text: String,
    pub added_rows: Vec<DeltaRow>,
    pub mod_rows: Vec<DeltaRow>,
    pub removed_rows: Vec<DeltaRow>,
}

impl DeltaViewData {
    /// Raw rows for the given category index. Uses [`DeltaType::from_index`].
    #[must_use]
    pub fn rows_by_index(&self, idx: usize) -> &[DeltaRow] {
        match DeltaType::from_index(idx) {
            DeltaType::Added => &self.added_rows,
            DeltaType::Mod => &self.mod_rows,
            DeltaType::Removed => &self.removed_rows,
        }
    }
}

/// Result from background right-pane resolve.
#[derive(Debug)]
pub struct RightPaneAsyncReady {
    pub generation: u64,
    pub path: String,
    pub content: RightPaneContent,
    pub disk_cache: Option<ViewerDiskContentCache>,
}

#[derive(Clone, Debug, Default)]
pub struct SnapshotEntryMeta {
    pub path: Option<String>,
    pub category: Option<String>,
    pub size: Option<u64>,
    pub mtime_ns: Option<i64>,
    pub has_zahir_json: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RightPaneContentDerived {
    pub abs_path: Option<PathBuf>,
    pub can_open: bool,
    pub offer_enhance_zahir: bool,
    pub offer_enhance_directory_policy: bool,
    pub embedded_cover_raster: Option<Vec<u8>>,
}

/// Text to show in the right pane for the current selection.
#[derive(Default, Clone, Debug)]
pub struct RightPaneContent {
    pub templates: String,
    pub metadata: Option<String>,
    pub writing: Option<String>,
    /// File/tree preview body; shared by reference for async highlight jobs (cheap `Arc::clone`).
    pub viewer: Option<Arc<str>>,
    /// When set with a directory tree [`viewer`] body, shown above the tree (bold + italic in the UI).
    pub viewer_directory_policy_line: Option<String>,
    pub snap_meta: SnapshotEntryMeta,
    pub derived: RightPaneContentDerived,
}

impl RightPaneContent {
    /// Empty right-pane content (e.g. Delta mode has no selection-based viewer).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Zahir / viewer routing type from snapshot `category` (see [`crate::integrations::file_type_from_metadata_name`]).
    #[must_use]
    pub fn zahir_file_type(&self) -> Option<ZahirFT> {
        file_type_from_metadata_name(self.snap_meta.category.as_deref().unwrap_or(""))
    }

    /// Snapshot `category` column as [`UblxDbCategory`] (same classification as the DB / [`UblxDbCategory::get_category_for_path`]).
    #[must_use]
    pub fn ublx_db_category(&self) -> UblxDbCategory {
        UblxDbCategory::from_snapshot_category(self.snap_meta.category.as_deref().unwrap_or(""))
    }
}
