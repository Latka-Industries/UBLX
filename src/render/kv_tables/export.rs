//! Serializable table sections for HTTP / web UI (same parse path as TUI draw).

use serde::Serialize;
use serde_json::Value;

use crate::config::ColumnStatsDisplay;

use super::format;
use super::parse_ctx::KvParseCtx;
use super::sections::{ContentsSection, Section, TreeNode, parse_json_sections_with_ctx};

/// One rendered Metadata / Writing section for clients that cannot draw ratatui tables.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SectionView {
    KeyValue {
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        sub_title: bool,
        rows: Vec<KvRowView>,
    },
    Contents {
        title: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        sub_title: bool,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    SingleColumnList {
        title: String,
        values: Vec<String>,
    },
    /// Nested schema tree (collapsible in the web UI).
    Tree {
        title: String,
        roots: Vec<TreeNodeView>,
    },
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct KvRowView {
    pub key: String,
    pub value: String,
}

/// One schema tree node for HTTP / web UI.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct TreeNodeView {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TreeNodeView>,
    /// Collapsible even with no children (directories).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub branch: bool,
}

/// Parse Metadata / Writing JSON into [`SectionView`]s (abbrev/full/none via `typed_column_tables`).
#[must_use]
pub fn parse_json_to_views(
    json: &str,
    typed_column_tables: ColumnStatsDisplay,
) -> Vec<SectionView> {
    let ctx = KvParseCtx::new(format::DEFAULT_MAX_ARRAY_INLINE, typed_column_tables);
    let sections = parse_json_sections_with_ctx(json, ctx);
    sections_to_views(&sections, ctx.max_array_inline)
}

/// Materialize Contents cells with the same [`format::format_value`] rules as TUI draw.
#[must_use]
pub fn sections_to_views(sections: &[Section], max_array_inline: usize) -> Vec<SectionView> {
    sections
        .iter()
        .map(|s| section_to_view(s, max_array_inline))
        .collect()
}

fn section_to_view(section: &Section, max_array_inline: usize) -> SectionView {
    match section {
        Section::KeyValue(kv) => SectionView::KeyValue {
            title: kv.title.clone(),
            sub_title: kv.sub_title,
            rows: kv
                .rows
                .iter()
                .map(|(k, v)| KvRowView {
                    key: k.clone(),
                    value: v.clone(),
                })
                .collect(),
        },
        Section::Contents(c) => SectionView::Contents {
            title: c.title.clone(),
            sub_title: c.sub_title,
            columns: c.columns.clone(),
            rows: contents_rows(c, max_array_inline),
        },
        Section::SingleColumnList(list) => SectionView::SingleColumnList {
            title: list.title.clone(),
            values: list.values.clone(),
        },
        Section::Tree(tree) => SectionView::Tree {
            title: tree.title.clone(),
            roots: tree.roots.iter().map(tree_node_to_view).collect(),
        },
    }
}

pub fn tree_node_to_view(node: &TreeNode) -> TreeNodeView {
    TreeNodeView {
        label: node.label.clone(),
        value: node.value.clone(),
        children: node.children.iter().map(tree_node_to_view).collect(),
        branch: node.branch,
    }
}

fn contents_rows(section: &ContentsSection, max_array_inline: usize) -> Vec<Vec<String>> {
    section
        .entries
        .iter()
        .filter_map(|entry| entry.as_object())
        .map(|obj| {
            section
                .column_keys
                .iter()
                .map(|k| {
                    obj.get(k).map_or_else(
                        || "—".to_string(),
                        |v: &Value| format::format_value(v, k, max_array_inline),
                    )
                })
                .collect()
        })
        .collect()
}
