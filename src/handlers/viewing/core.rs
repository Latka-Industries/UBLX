//! Load and fill right-pane content for the current selection (tree, file viewer, zahir JSON).
//! Moved from layout so "get the data that goes into the view" lives with other handlers.

use serde_json::{self, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::config::{EnhancePolicy, UblxOpts};
use crate::engine::db_ops;
use crate::integrations::{ZahirFT, file_type_from_metadata_name, is_zarr_category_str};
use crate::layout::setup::{
    CATEGORY_DIRECTORY, RightPaneContent, RightPaneContentDerived, SectionedPreview,
    SnapshotEntryMeta, TuiRow, UblxState, ViewData, ViewerDiskContentCache,
};
use crate::render::kv_tables::WalkKeyVars;
use crate::ui::UI_STRINGS;
use crate::utils::{self, path_is_tet_file};

use super::zarrstore::{ZarrStoreRightPaneView, right_pane_from_zarr_tree_and_zahir};

/// Minified JSON is often one physical line, so `lines().count()` stays 1 while the pane wraps to
/// many rows → scrollbar / `content_width` thrash with async syntect. Valid JSON is replaced with
/// [`serde_json::to_string_pretty`] so line estimates match the rendered body.
fn pretty_json_viewer_body(s: String) -> String {
    if s.is_empty() {
        return s;
    }
    match serde_json::from_str::<Value>(&s) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or(s),
        Err(_) => s,
    }
}

/// Run `tree` with no in-memory cache (shared with [`super::zarrstore`] for async Zarr stores).
pub(crate) fn tree_subprocess_for_path(full_path_ref: &Path) -> Result<String, String> {
    match Command::new("tree").arg(full_path_ref).output() {
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into_owned()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(format!(
                "tree failed: {}",
                stderr.trim().lines().next().unwrap_or("unknown")
            ))
        }
        Err(e) => Err(format!("tree not available: {e}")),
    }
}

/// Caps for structured directory trees (web collapsible Viewer).
struct DirTreeLimits;
impl DirTreeLimits {
    const MAX_DEPTH: u32 = 8;
    const MAX_NODES: usize = 2_000;
}

/// Walk `root` into nested [`TreeNode`]s for the web Viewer (TUI still uses `tree` text).
#[must_use]
pub fn directory_tree_nodes(root: &Path) -> Vec<crate::render::kv_tables::TreeNode> {
    let mut budget = DirTreeLimits::MAX_NODES;
    walk_dir_nodes(root, 0, &mut budget)
}

fn walk_dir_nodes(
    dir: &Path,
    depth: u32,
    budget: &mut usize,
) -> Vec<crate::render::kv_tables::TreeNode> {
    use crate::render::kv_tables::TreeNode;

    if depth >= DirTreeLimits::MAX_DEPTH || *budget == 0 {
        return Vec::new();
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| {
        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
        (!is_dir, e.file_name())
    });

    let mut out = Vec::new();
    for ent in entries {
        if *budget == 0 {
            out.push(TreeNode {
                label: "…".into(),
                value: Some("truncated".into()),
                children: Vec::new(),
                branch: false,
            });
            break;
        }
        *budget = budget.saturating_sub(1);
        let name = ent.file_name().to_string_lossy().into_owned();
        let path = ent.path();
        let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            let children = walk_dir_nodes(&path, depth + 1, budget);
            out.push(TreeNode {
                label: name,
                value: None,
                children,
                branch: true,
            });
        } else {
            out.push(TreeNode {
                label: name,
                value: None,
                children: Vec::new(),
                branch: false,
            });
        }
    }
    out
}

pub(crate) fn zahir_json_string_for_path(db_path: &Path, rel_path: &str) -> String {
    db_ops::load_zahir_json_for_path(db_path, rel_path)
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Run `tree` on `full_path`; use cached result if keyed by `path`. Updates `state.cached_tree`.
fn tree_for_path(state_mut: &mut UblxState, path_ref: &str, full_path_ref: &Path) -> String {
    if let Some((cached_path, text)) = state_mut.cached_tree.as_ref()
        && cached_path == path_ref
    {
        return text.clone();
    }
    match tree_subprocess_for_path(full_path_ref) {
        Ok(text) => {
            state_mut.cached_tree = Some((path_ref.to_string(), text.clone()));
            text
        }
        Err(e) => {
            state_mut.cached_tree = None;
            e
        }
    }
}

pub(crate) fn directory_tree_policy_line(ublx_opts: &UblxOpts, rel_path: &str) -> String {
    let label = UI_STRINGS.pane.current_enhance_policy_label;
    let value = match ublx_opts.matching_enhance_policy(rel_path) {
        Some(EnhancePolicy::Auto) => UI_STRINGS.pane.directory_policy_auto,
        Some(EnhancePolicy::Manual) => UI_STRINGS.pane.directory_policy_manual,
        None => {
            if ublx_opts.enable_enhance_all {
                UI_STRINGS.pane.directory_policy_inherit_auto
            } else {
                UI_STRINGS.pane.directory_policy_inherit_manual
            }
        }
    };
    format!("{label}: {value}")
}

fn tree_viewer(
    tree_str: String,
    rel_path: &str,
    abs_path: PathBuf,
    offer_directory_policy: bool,
    mtime_ns: Option<i64>,
    viewer_has_zahir_json: bool,
    policy_line: Option<String>,
) -> RightPaneContent {
    RightPaneContent {
        templates: String::new(),
        metadata: None,
        writing: None,
        viewer: Some(Arc::from(tree_str)),
        viewer_directory_policy_line: policy_line,
        snap_meta: SnapshotEntryMeta {
            path: Some(rel_path.to_string()),
            category: Some(CATEGORY_DIRECTORY.to_string()),
            size: Some(0),
            mtime_ns,
            has_zahir_json: viewer_has_zahir_json,
        },
        derived: RightPaneContentDerived {
            abs_path: Some(abs_path),
            can_open: false,
            offer_enhance_zahir: false,
            offer_enhance_directory_policy: offer_directory_policy,
            embedded_cover_raster: None,
        },
    }
}

/// Inputs for [`resolve_non_directory_right_pane`]: selected row + resolved path + DB mtime.
struct NonDirectoryPaneInputs<'a> {
    path: &'a str,
    category: &'a str,
    size: u64,
    full_path: PathBuf,
    viewer_mtime_ns: Option<i64>,
    enable_enhance_all: bool,
}

/// Arguments for [`build_non_directory_right_pane_inner`].
pub struct NonDirectoryRightPaneBuild<'a> {
    pub db_path: &'a Path,
    pub path: &'a str,
    pub category: &'a str,
    pub size: u64,
    pub full_path: &'a Path,
    pub viewer_mtime_ns: Option<i64>,
    pub enable_enhance_all: bool,
    pub disk_cache_hint: Option<&'a ViewerDiskContentCache>,
}

/// Load viewer text, cover raster, openability, and optional new disk-cache row (cache miss only).
fn resolve_viewer_disk_payload(
    path: &str,
    category: &str,
    full_path: &Path,
    viewer_zahir_type: Option<ZahirFT>,
    meta_opt_ref: Option<&std::fs::Metadata>,
    disk_cache_hit: Option<&ViewerDiskContentCache>,
) -> (
    Option<String>,
    Option<Vec<u8>>,
    bool,
    Option<ViewerDiskContentCache>,
) {
    if let Some(c) = disk_cache_hit {
        let viewer_str = match viewer_zahir_type {
            Some(ZahirFT::Json) => c.viewer_str.clone().map(pretty_json_viewer_body),
            _ => c.viewer_str.clone(),
        };
        return (
            viewer_str,
            c.embedded_cover_raster.clone(),
            c.viewer_can_open,
            None,
        );
    }

    let embedded_cover = match viewer_zahir_type {
        Some(ft @ (ZahirFT::Audio | ZahirFT::Epub)) => utils::try_extract_cover(full_path, ft),
        _ => None,
    };
    let viewer_str = if embedded_cover.is_some() {
        Some(String::new())
    } else {
        utils::file_content_for_viewer(full_path, viewer_zahir_type)
    };
    let viewer_str = match viewer_zahir_type {
        Some(ZahirFT::Json) => viewer_str.map(pretty_json_viewer_body),
        _ => viewer_str,
    };
    let viewer_can_open = !utils::is_likely_binary(full_path)
        || matches!(
            viewer_zahir_type,
            Some(
                ZahirFT::Image
                    | ZahirFT::Pdf
                    | ZahirFT::Video
                    | ZahirFT::Audio
                    | ZahirFT::Epub
                    | ZahirFT::Tetration
            )
        )
        || path_is_tet_file(full_path);
    let cache = meta_opt_ref.map(|meta| ViewerDiskContentCache {
        rel_path: path.to_string(),
        category: category.to_string(),
        file_len: meta.len(),
        modified: meta.modified().ok(),
        viewer_str: viewer_str.clone(),
        embedded_cover_raster: embedded_cover.clone(),
        viewer_can_open,
    });
    (viewer_str, embedded_cover, viewer_can_open, cache)
}

pub(crate) fn zahir_derived_pane_fields(
    zahir_json: &str,
    enable_enhance_all: bool,
) -> (String, Option<String>, Option<String>, bool) {
    if zahir_json.is_empty() {
        return (String::new(), None, None, !enable_enhance_all);
    }
    match serde_json::from_str::<Value>(zahir_json) {
        Ok(v) => {
            let s = sectioned_preview_from_zahir(&v);
            (s.templates, s.metadata, s.writing, false)
        }
        _ => (String::new(), None, None, false),
    }
}

/// Build file-row right pane (not a directory on disk): disk read, optional cover, DB zahir JSON.
/// `disk_cache_hint` is usually [`UblxState::viewer_disk_cache`] on the UI thread; background workers pass [`None`].
#[must_use]
pub fn build_non_directory_right_pane_inner(
    input: &NonDirectoryRightPaneBuild<'_>,
) -> (RightPaneContent, Option<ViewerDiskContentCache>) {
    let _perf = utils::PerfGuard::new("right_pane.non_directory_inner");
    let NonDirectoryRightPaneBuild {
        db_path: db_path_ref,
        path,
        category,
        size,
        full_path,
        viewer_mtime_ns,
        enable_enhance_all,
        disk_cache_hint,
    } = input;
    let size = *size;
    let viewer_mtime_ns = *viewer_mtime_ns;
    let enable_enhance_all = *enable_enhance_all;

    let viewer_zahir_type = file_type_from_metadata_name(category)
        .or_else(|| path_is_tet_file(full_path).then_some(ZahirFT::Tetration));

    let meta_opt = std::fs::metadata(full_path).ok();
    let disk_cache_hit = meta_opt
        .as_ref()
        .and_then(|meta| disk_cache_hint.filter(|c| c.matches(path, category, meta)));

    let (viewer_str, embedded_cover, viewer_can_open, new_disk_cache) = resolve_viewer_disk_payload(
        path,
        category,
        full_path,
        viewer_zahir_type,
        meta_opt.as_ref(),
        disk_cache_hit,
    );

    let viewer_byte_size = viewer_str.as_ref().map(|_| size);
    let zahir_json = zahir_json_string_for_path(db_path_ref, path);

    let (templates, metadata, writing, viewer_offer_enhance_zahir) =
        zahir_derived_pane_fields(&zahir_json, enable_enhance_all);

    let content = RightPaneContent {
        templates,
        metadata,
        writing,
        viewer: viewer_str.map(Arc::from),
        viewer_directory_policy_line: None,
        snap_meta: SnapshotEntryMeta {
            path: Some(path.to_string()),
            category: Some(category.to_string()),
            size: viewer_byte_size,
            mtime_ns: viewer_mtime_ns,
            has_zahir_json: !zahir_json.is_empty(),
        },
        derived: RightPaneContentDerived {
            abs_path: Some(full_path.to_path_buf()),
            can_open: viewer_can_open,
            offer_enhance_zahir: viewer_offer_enhance_zahir,
            offer_enhance_directory_policy: false,
            embedded_cover_raster: embedded_cover.clone(),
        },
    };
    (content, new_disk_cache)
}

/// File or non-directory row: tree if the path is a directory on disk; otherwise viewer + optional zahir JSON.
fn resolve_non_directory_right_pane(
    state_mut: &mut UblxState,
    db_path_ref: &Path,
    inputs: NonDirectoryPaneInputs<'_>,
    ublx_opts: &UblxOpts,
) -> RightPaneContent {
    let NonDirectoryPaneInputs {
        path,
        category,
        size,
        full_path,
        viewer_mtime_ns,
        enable_enhance_all,
    } = inputs;
    if full_path.is_dir() {
        let tree_str = tree_for_path(state_mut, path, &full_path);
        let zahir_json = zahir_json_string_for_path(db_path_ref, path);
        if is_zarr_category_str(category) {
            let view = ZarrStoreRightPaneView {
                path,
                category,
                size,
                full_path,
                viewer_mtime_ns,
                enable_enhance_all,
                ublx_opts,
            };
            return right_pane_from_zarr_tree_and_zahir(tree_str, &zahir_json, &view);
        }
        let policy_line = Some(directory_tree_policy_line(ublx_opts, path));
        return tree_viewer(
            tree_str,
            path,
            full_path,
            false,
            viewer_mtime_ns,
            !zahir_json.is_empty(),
            policy_line,
        );
    }
    state_mut.cached_tree = None;
    let hint = state_mut.viewer_disk_cache.as_ref();
    let (content, new_cache) = build_non_directory_right_pane_inner(&NonDirectoryRightPaneBuild {
        db_path: db_path_ref,
        path,
        category,
        size,
        full_path: &full_path,
        viewer_mtime_ns,
        enable_enhance_all,
        disk_cache_hint: hint,
    });
    if let Some(c) = new_cache {
        state_mut.viewer_disk_cache = Some(c);
    } else if std::fs::metadata(&full_path).is_err() {
        state_mut.viewer_disk_cache = None;
    }
    content
}

/// Resolve right-pane strings from current selection: directory => tree; file => zahir sections.
/// `zahir_json` is loaded from the DB only for the selected row (lazy load).
/// For snapshot mode pass `Some(all_rows)`; for delta mode pass `None` (view holds rows).
pub fn resolve_right_pane_content(
    state_mut: &mut UblxState,
    dir_to_ublx_ref: &Path,
    db_path_ref: &Path,
    view_ref: &ViewData,
    all_rows_ref: Option<&[TuiRow]>,
    ublx_opts: &UblxOpts,
) -> RightPaneContent {
    let selected: Option<&TuiRow> = state_mut
        .panels
        .content_state
        .selected()
        .and_then(|i| view_ref.row_at(i, all_rows_ref));
    if let Some((path, category, size)) = selected {
        let path: &str = path.as_str();
        let full_path = utils::resolve_under_root(dir_to_ublx_ref, path);
        let viewer_mtime_ns = db_ops::load_mtime_for_path(db_path_ref, path)
            .ok()
            .flatten();
        if *category == CATEGORY_DIRECTORY {
            let tree_str = tree_for_path(state_mut, path, &full_path);
            let has_zahir = db_ops::load_zahir_json_for_path(db_path_ref, path)
                .ok()
                .flatten()
                .is_some_and(|s| !s.is_empty());
            let policy_line = Some(directory_tree_policy_line(ublx_opts, path));
            tree_viewer(
                tree_str,
                path,
                full_path,
                true,
                viewer_mtime_ns,
                has_zahir,
                policy_line,
            )
        } else {
            resolve_non_directory_right_pane(
                state_mut,
                db_path_ref,
                NonDirectoryPaneInputs {
                    path,
                    category,
                    size: *size,
                    full_path,
                    viewer_mtime_ns,
                    enable_enhance_all: ublx_opts.enable_enhance_all,
                },
                ublx_opts,
            )
        }
    } else {
        state_mut.cached_tree = None;
        state_mut.viewer_disk_cache = None;
        RightPaneContent::empty()
    }
}

/// Drop zahirscan placeholder image dimensions (`width`/`height` both `0`).
///
/// SVG (and other formats `image` cannot header-probe) currently land as `0×0` in
/// `image_metadata`. Preview sizing ignores these fields; showing them in Metadata is noise.
/// Permanent fix belongs in zahirscan (parse SVG attrs or omit unknowns).
fn scrub_placeholder_image_dimensions(meta: &mut serde_json::Map<String, Value>) {
    let both_zero = matches!(
        (meta.get("width"), meta.get("height")),
        (Some(Value::Number(w)), Some(Value::Number(h)))
            if w.as_u64() == Some(0) && h.as_u64() == Some(0)
    );
    if !both_zero {
        return;
    }
    meta.remove("width");
    meta.remove("height");
    if meta.get("aspect_ratio").is_some_and(Value::is_null) {
        meta.remove("aspect_ratio");
    }
}

/// Build `SectionedPreview` (templates, metadata, writing) from zahir JSON value.
#[must_use]
pub fn sectioned_preview_from_zahir(value_ref: &serde_json::Value) -> SectionedPreview {
    let templates = value_ref
        .get("templates")
        .and_then(|t| serde_json::to_string_pretty(t).ok())
        .filter(|s| !s.is_empty() && s != "null" && s != "[]")
        .unwrap_or_default();

    let metadata = value_ref.as_object().and_then(|obj| {
        let root_file_type = obj.get(WalkKeyVars::FILE_TYPE);
        let parts: Vec<String> = obj
            .iter()
            .filter(|(k, _)| k.ends_with(WalkKeyVars::METADATA))
            .filter_map(|(_, v)| {
                let merged = match (root_file_type, v.as_object()) {
                    (Some(ft), Some(meta)) => {
                        let mut m = meta.clone();
                        scrub_placeholder_image_dimensions(&mut m);
                        m.entry(WalkKeyVars::FILE_TYPE.to_string())
                            .or_insert_with(|| ft.clone());
                        Value::Object(m)
                    }
                    (_, Some(meta)) => {
                        let mut m = meta.clone();
                        scrub_placeholder_image_dimensions(&mut m);
                        Value::Object(m)
                    }
                    _ => v.clone(),
                };
                serde_json::to_string_pretty(&merged).ok()
            })
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    });

    let writing = value_ref
        .get("writing_footprint")
        .and_then(|w| serde_json::to_string_pretty(w).ok());

    SectionedPreview {
        templates,
        metadata,
        writing,
    }
}
