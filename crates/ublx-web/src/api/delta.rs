//! Delta catalog types and fetch.

use serde::Deserialize;

use super::format::format_timestamp_ns;
use super::http::get_json;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct DeltaRow {
    pub created_ns: i64,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub delta_type: String,
}

/// Left-pane delta type (API: `added` / `mod` / `removed`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DeltaKind {
    #[default]
    Added,
    Modified,
    Removed,
}

impl DeltaKind {
    pub(crate) const ALL: [Self; 3] = [Self::Added, Self::Modified, Self::Removed];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Modified => "Modified",
            Self::Removed => "Removed",
        }
    }

    pub(crate) fn api_type(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "mod",
            Self::Removed => "removed",
        }
    }

    pub(crate) fn css_class(self) -> &'static str {
        match self {
            Self::Added => "delta-added",
            Self::Modified => "delta-mod",
            Self::Removed => "delta-removed",
        }
    }
}

/// Grouped `/delta` payload for the Delta mode panes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DeltaCatalog {
    pub rows: Vec<DeltaRow>,
}

impl DeltaCatalog {
    pub(crate) fn rows_for(&self, kind: DeltaKind) -> Vec<&DeltaRow> {
        let t = kind.api_type();
        self.rows
            .iter()
            .filter(|r| r.delta_type.eq_ignore_ascii_case(t))
            .collect()
    }

    /// Distinct `created_ns` values, newest first — same idea as TUI snapshot overview.
    pub(crate) fn overview_text(&self) -> String {
        let mut stamps: Vec<i64> = self.rows.iter().map(|r| r.created_ns).collect();
        stamps.sort_unstable_by(|a, b| b.cmp(a));
        stamps.dedup();
        let count = stamps.len();
        let mut lines = vec![
            String::new(),
            format!("{count} snapshot(s) (sorted by time; newest first):"),
            String::new(),
        ];
        for ns in stamps {
            lines.push(format!("  • {}", format_timestamp_ns(ns)));
        }
        lines.join("\n")
    }
}

pub(crate) async fn fetch_delta_catalog() -> DeltaCatalog {
    let rows = get_json::<Vec<DeltaRow>>("/delta")
        .await
        .unwrap_or_default();
    DeltaCatalog { rows }
}
