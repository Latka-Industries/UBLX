//! Parallel iteration thresholds. Used by layout filter, user-selected mode, CSV viewer,
//! `kv_tables` sections, tables natural widths, and markdown to decide when to use rayon.

#[derive(Clone, Copy, Debug)]
pub struct PrettyTablePrep {
    pub body_rows: usize,
    pub maxlens_min_cols: usize,
    pub maxlens_min_body_rows: usize,
}

/// Thresholds above which we switch to parallel iteration for specific operations.
#[derive(Clone, Copy, Debug)]
pub struct Parallel {
    /// Snapshot content indices: filter rows by category + fuzzy search. (`modules::search::content_indices_for_view`)
    pub content_indices: usize,
    /// Categories that have ≥1 row fuzzy-matching search. (`modules::search::categories_for_search`)
    pub categories_for_search: usize,
    /// Delta rows filtered by fuzzy path match. (`modules::search::filter_delta_rows`)
    pub delta_rows: usize,
    /// Duplicates groups or Lenses names filtered by search. (`app::user_selected`)
    pub user_selected_filter: usize,
    /// CSV / comfy-table: parallel truncate cells when row count ≥ this. (`render::viewers::pretty_tables::truncate_all_cells`)
    pub csv_truncate: usize,
    /// Comfy-table [`prepare_multiline_grid`](crate::render::viewers::pretty_tables::prepare_multiline_grid): parallel per-body-row wrap/pad when body rows ≥ this.
    pub pretty_tables_prep: PrettyTablePrep,
    /// JSON metadata: parse each "\n\n" blob in parallel. (`render::kv_tables::sections::parse_json_sections`)
    pub json_sections_blobs: usize,
    /// Contents table: natural column widths over visible rows (`kv_tables::ratatui_table`, internal).
    pub contents_natural_widths: usize,
    /// Markdown viewer: render blocks to lines in parallel. (`render::viewers::markdown::MarkdownDoc::to_text`)
    pub markdown_blocks: usize,
    /// Snapshot insert: parallelize preparation (`path_str`, category, `zahir_json`, `mtime_ns`, size, hash); DB write is ordered batched multi-row `INSERT`s.
    /// Owned by `ublx_catalog::db_ops::SNAPSHOT_INSERT_PREP_PARALLEL` (used by `insert_results_into_snapshot`); mirrored here for the tuning overview.
    pub snapshot_insert_prep: usize,
    /// Paths that need zahir: filter nefax by size > 0 and mtime/changed. (`engine::orchestrator::paths_needing_zahir`)
    pub paths_needing_zahir: usize,
}

/// Default parallel thresholds used across the app.
pub const PARALLEL: Parallel = Parallel {
    content_indices: 5000,
    categories_for_search: 100,
    delta_rows: 2000,
    user_selected_filter: 500,
    csv_truncate: 100,
    pretty_tables_prep: PrettyTablePrep {
        body_rows: 64,
        maxlens_min_cols: 4,
        maxlens_min_body_rows: 32,
    },
    json_sections_blobs: 20,
    contents_natural_widths: 1000,
    markdown_blocks: 100,
    snapshot_insert_prep: ublx_catalog::db_ops::SNAPSHOT_INSERT_PREP_PARALLEL,
    paths_needing_zahir: 20_000,
};
