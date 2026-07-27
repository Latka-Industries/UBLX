//! Snapshot-mode view data: filtered categories and contents from search + selection.

use std::collections::HashMap;

use crate::layout::setup;
use crate::modules::catalog_filter::{self, fuzzy_matches_field};
use crate::utils::{clamp_selection, clamp_selection_opt};

fn sort_indices_by_mode(
    indices: &mut [usize],
    all_rows: &[setup::TuiRow],
    mode: setup::ContentSort,
    mtimes_by_path: Option<&HashMap<String, Option<i64>>>,
) {
    match mode.snapshot_key {
        setup::SnapshotSortKey::Name => {
            indices.sort_unstable_by(|a, b| {
                all_rows[*a]
                    .0
                    .cmp(&all_rows[*b].0)
                    .then_with(|| all_rows[*a].2.cmp(&all_rows[*b].2))
            });
        }
        setup::SnapshotSortKey::Size => {
            indices.sort_unstable_by(|a, b| {
                all_rows[*a]
                    .2
                    .cmp(&all_rows[*b].2)
                    .then_with(|| all_rows[*a].0.cmp(&all_rows[*b].0))
            });
        }
        setup::SnapshotSortKey::Mod => {
            indices.sort_unstable_by(|a, b| {
                let a_row = &all_rows[*a];
                let b_row = &all_rows[*b];
                let a_mtime = mtimes_by_path
                    .and_then(|m| m.get(&a_row.0))
                    .copied()
                    .flatten()
                    .unwrap_or(i64::MIN);
                let b_mtime = mtimes_by_path
                    .and_then(|m| m.get(&b_row.0))
                    .copied()
                    .flatten()
                    .unwrap_or(i64::MIN);
                a_mtime.cmp(&b_mtime).then_with(|| a_row.0.cmp(&b_row.0))
            });
        }
    }
    if mode.snapshot_dir == setup::SortDirection::Desc {
        indices.reverse();
    }
}

fn sort_rows_by_mode(
    rows: &mut [setup::TuiRow],
    mode: setup::ContentSort,
    main_mode: setup::MainMode,
) {
    if main_mode == setup::MainMode::Lenses {
        return;
    }
    match mode.snapshot_key {
        setup::SnapshotSortKey::Name => rows.sort_unstable_by(|a, b| a.0.cmp(&b.0)),
        setup::SnapshotSortKey::Size => {
            rows.sort_unstable_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
        }
        setup::SnapshotSortKey::Mod => {}
    }
    if mode.snapshot_dir == setup::SortDirection::Desc {
        rows.reverse();
    }
}

/// Resolve the selected category string from the category list index (0 = "All").
fn selected_category(filtered_categories: &[String], category_idx: usize) -> Option<&str> {
    if category_idx == 0 {
        None
    } else {
        filtered_categories
            .get(category_idx - 1)
            .map(String::as_str)
    }
}

/// Clamp category list selection to valid range and update state.
fn clamp_category_selection(state_mut: &mut setup::UblxState, category_list_len: usize) {
    if category_list_len == 0 {
        return;
    }
    let category_idx = state_mut.panels.category_state.selected().unwrap_or(0);
    state_mut
        .panels
        .category_state
        .select(Some(clamp_selection(category_idx, category_list_len)));
}

/// Clamp content list selection to valid range and update state.
fn clamp_content_selection(state_mut: &mut setup::UblxState, content_len: usize) {
    let current = state_mut.panels.content_state.selected().unwrap_or(0);
    if let Some(sel) = clamp_selection_opt(current, content_len) {
        state_mut.panels.content_state.select(Some(sel));
    } else {
        state_mut.panels.content_state.select(None);
    }
}

/// Filter content rows by search query on the path (first element of [`setup::TuiRow`]). Shared by Duplicates and Lenses.
pub fn filter_contents_by_search(
    rows: Vec<setup::TuiRow>,
    search_query: &str,
) -> Vec<setup::TuiRow> {
    if search_query.is_empty() {
        rows
    } else {
        rows.into_iter()
            .filter(|(path, _, _)| fuzzy_matches_field(path, search_query))
            .collect()
    }
}

/// Build [`ViewData`] from filtered category names and content rows (`DeltaRows`). Shared by Duplicates and Lenses.
pub fn build_user_selected_mode_view_data(
    main_mode: setup::MainMode,
    filtered_categories: Vec<String>,
    mut contents: Vec<setup::TuiRow>,
    sort: setup::ContentSort,
) -> setup::ViewData {
    sort_rows_by_mode(&mut contents, sort, main_mode);
    let category_list_len = filtered_categories.len().max(1);
    let content_len = contents.len();
    setup::ViewData {
        filtered_categories,
        contents: setup::ViewContents::DeltaRows(contents),
        category_list_len,
        content_len,
    }
}

/// Clamp category and content list selection from a [`ViewData`]. Shared by Duplicates and Lenses (and any two-pane list mode).
pub fn clamp_two_pane_selection(state_mut: &mut setup::UblxState, view_ref: &setup::ViewData) {
    clamp_category_selection(state_mut, view_ref.category_list_len);
    clamp_content_selection(state_mut, view_ref.content_len);
}

/// Reset preview scroll when category or content selection changes.
fn sync_preview_scroll(state_mut: &mut setup::UblxState, category_idx: usize) {
    let content_sel = state_mut.panels.content_state.selected();
    let preview_key = (category_idx, content_sel);
    if state_mut.panels.prev_preview_key.as_ref() != Some(&preview_key) {
        state_mut.panels.preview_scroll = 0;
        state_mut.panels.template_list.select(Some(0));
        state_mut.panels.prev_preview_key = Some(preview_key);
    }
}

/// Compute filtered categories and contents from search + category selection; clamp list
/// selection and reset preview scroll when selection changes.
pub fn build_view_data(
    state_mut: &mut setup::UblxState,
    categories: &[String],
    all_rows: &[setup::TuiRow],
    mtimes_by_path: Option<&HashMap<String, Option<i64>>>,
) -> setup::ViewData {
    let search_query = state_mut.search.query.trim();
    let filtered_categories =
        catalog_filter::categories_for_search(categories, all_rows, search_query);
    let category_idx = state_mut.panels.category_state.selected().unwrap_or(0);
    let selected_category = selected_category(&filtered_categories, category_idx);
    let mut contents_indices =
        catalog_filter::content_indices_for_view(all_rows, selected_category, search_query);
    if search_query.is_empty() {
        sort_indices_by_mode(
            &mut contents_indices,
            all_rows,
            state_mut.panels.content_sort,
            mtimes_by_path,
        );
    }

    let category_list_len = 1 + filtered_categories.len();
    let content_len = contents_indices.len();

    clamp_category_selection(state_mut, category_list_len);
    clamp_content_selection(state_mut, content_len);
    sync_preview_scroll(state_mut, category_idx);

    setup::ViewData {
        filtered_categories,
        contents: setup::ViewContents::SnapshotIndices(contents_indices),
        category_list_len,
        content_len,
    }
}
