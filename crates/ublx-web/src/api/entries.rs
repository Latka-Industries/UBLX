//! Catalog entry rows, Zahir section views, and detail fetch.

use serde::Deserialize;
use serde_json::Value;

use super::http::{encode_entry_path, get_json};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct EntryRow {
    pub path: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub mtime_ns: Option<i64>,
    #[serde(default)]
    pub zahir: Option<Value>,
    /// Host-parsed Metadata tables (`kv_tables`); present when `?zahir=1`.
    #[serde(default)]
    pub metadata_tables: Option<Vec<SectionView>>,
    /// Host-parsed Writing tables.
    #[serde(default)]
    pub writing_tables: Option<Vec<SectionView>>,
}

/// One Metadata / Writing section from serve (`SectionView` export).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SectionView {
    KeyValue {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        sub_title: bool,
        #[serde(default)]
        rows: Vec<KvRowView>,
    },
    Contents {
        title: String,
        #[serde(default)]
        sub_title: bool,
        #[serde(default)]
        columns: Vec<String>,
        #[serde(default)]
        rows: Vec<Vec<String>>,
    },
    SingleColumnList {
        title: String,
        #[serde(default)]
        values: Vec<String>,
    },
    Tree {
        title: String,
        #[serde(default)]
        roots: Vec<TreeNodeView>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct KvRowView {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct TreeNodeView {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub children: Vec<TreeNodeView>,
    #[serde(default)]
    pub branch: bool,
}

/// Right-pane payload derived from `/entries/{path}?zahir=1` (mirrors TUI section split).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EntryDetail {
    pub path: String,
    pub category: String,
    pub size: u64,
    pub mtime_ns: Option<i64>,
    pub templates: String,
    pub metadata: Option<String>,
    pub writing: Option<String>,
    pub metadata_tables: Vec<SectionView>,
    pub writing_tables: Vec<SectionView>,
}

impl EntryDetail {
    pub(crate) fn from_row(row: EntryRow) -> Self {
        let sections = sections_from_zahir(row.zahir.as_ref());
        Self {
            path: row.path,
            category: row.category,
            size: row.size,
            mtime_ns: row.mtime_ns,
            templates: sections.templates,
            metadata: sections.metadata,
            writing: sections.writing,
            metadata_tables: row.metadata_tables.unwrap_or_default(),
            writing_tables: row.writing_tables.unwrap_or_default(),
        }
    }

    pub(crate) fn has_templates(&self) -> bool {
        !self.templates.is_empty()
    }

    pub(crate) fn has_metadata(&self) -> bool {
        !self.metadata_tables.is_empty() || self.metadata.as_ref().is_some_and(|s| !s.is_empty())
    }

    pub(crate) fn has_writing(&self) -> bool {
        !self.writing_tables.is_empty() || self.writing.as_ref().is_some_and(|s| !s.is_empty())
    }
}

#[derive(Default)]
struct ZahirSections {
    templates: String,
    metadata: Option<String>,
    writing: Option<String>,
}

/// Same key rules as TUI `sectioned_preview_from_zahir` (simplified; no image scrub).
fn sections_from_zahir(zahir: Option<&Value>) -> ZahirSections {
    let Some(value) = zahir else {
        return ZahirSections::default();
    };

    let templates = value
        .get("templates")
        .and_then(|t| serde_json::to_string_pretty(t).ok())
        .filter(|s| !s.is_empty() && s != "null" && s != "[]")
        .unwrap_or_default();

    let metadata = value.as_object().and_then(|obj| {
        let root_file_type = obj.get("file_type");
        let parts: Vec<String> = obj
            .iter()
            .filter(|(k, _)| k.ends_with("_metadata"))
            .filter_map(|(_, v)| {
                let merged = match (root_file_type, v.as_object()) {
                    (Some(ft), Some(meta)) => {
                        let mut m = meta.clone();
                        m.entry("file_type".to_string())
                            .or_insert_with(|| ft.clone());
                        Value::Object(m)
                    }
                    (_, Some(meta)) => Value::Object(meta.clone()),
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

    let writing = value
        .get("writing_footprint")
        .and_then(|w| serde_json::to_string_pretty(w).ok());

    ZahirSections {
        templates,
        metadata,
        writing,
    }
}

fn entry_zahir_url(path: &str) -> String {
    format!("/entries/{}?zahir=1", encode_entry_path(path))
}

async fn fetch_entry_row_zahir(path: &str) -> Result<EntryRow, String> {
    get_json::<EntryRow>(&entry_zahir_url(path)).await
}

/// Fetch one catalog entry with Zahir JSON (`GET /entries/{path}?zahir=1`).
pub(crate) async fn fetch_entry_detail(path: &str) -> Result<EntryDetail, String> {
    Ok(EntryDetail::from_row(fetch_entry_row_zahir(path).await?))
}

/// Optional-path wrapper for middle-pane `LocalResource` detail loads.
pub(crate) async fn fetch_entry_detail_opt(path: Option<String>) -> Option<EntryDetail> {
    match path {
        Some(p) => fetch_entry_detail(&p).await.ok(),
        None => None,
    }
}

/// Raw Zahir JSON for clipboard (`GET /entries/{path}?zahir=1`).
pub(crate) async fn fetch_entry_zahir_raw(path: &str) -> Result<Option<Value>, String> {
    Ok(fetch_entry_row_zahir(path).await?.zahir)
}
