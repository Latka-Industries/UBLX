//! Serve/query-safe preview helpers (no ratatui). Extracted for THI-213 so
//! `handlers::viewing` TUI paths can be feature-gated without breaking HTTP content.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::layout::setup::SectionedPreview;
use crate::render::kv_tables::{TreeNode, WalkKeyVars};
use crate::render::templates;

/// Caps for structured directory trees (web collapsible Viewer).
struct DirTreeLimits;
impl DirTreeLimits {
    const MAX_DEPTH: u32 = 8;
    const MAX_NODES: usize = 2_000;
}

/// Walk `root` into nested [`TreeNode`]s for the web Viewer (TUI still uses `tree` text).
#[must_use]
pub fn directory_tree_nodes(root: &Path) -> Vec<TreeNode> {
    let mut budget = DirTreeLimits::MAX_NODES;
    walk_dir_nodes(root, 0, &mut budget)
}

fn walk_dir_nodes(dir: &Path, depth: u32, budget: &mut usize) -> Vec<TreeNode> {
    if depth >= DirTreeLimits::MAX_DEPTH || *budget == 0 {
        return Vec::new();
    }
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| {
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
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
        let is_dir = ent.file_type().is_ok_and(|t| t.is_dir());
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
pub fn sectioned_preview_from_zahir(value_ref: &Value) -> SectionedPreview {
    let template_views = templates::template_views_from_value(value_ref);
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
        template_views,
        metadata,
        writing,
    }
}
