//! Snapshot mode: categories · contents · right pane (+ catalog search filter).

use std::sync::Arc;

use leptos::prelude::*;

use crate::api::{EntryRow, fetch_entry_detail, get_json};
use crate::catalog_refresh::CatalogRefresh;
use crate::focus::{UiNav, install_list_nav, string_list_nav};
use crate::nav::MainMode;
use crate::panes::{EntryRightPane, PanelRow, PathsPane, ThreePane};
use crate::search::{CatalogSearch, filter_categories, filter_snapshot_paths, path_rows};
use crate::sort::{ContentSortCtx, sort_snapshot_rows};

/// Path + category + sort fields only (drop zahir payload from the sort hot path).
#[derive(Clone, PartialEq, Eq)]
struct SlimEntry {
    path: String,
    category: String,
    size: u64,
    mtime_ns: Option<i64>,
}

#[component]
pub(crate) fn SnapshotMode() -> impl IntoView {
    let search = CatalogSearch::expect();
    let refresh = CatalogRefresh::expect();
    let categories = LocalResource::new(move || {
        let _ = refresh.tick.get();
        async move {
            get_json::<Vec<String>>("/categories")
                .await
                .unwrap_or_default()
        }
    });
    // Full catalog once — needed so category visibility can see matches in other categories.
    let entries = LocalResource::new(move || {
        let _ = refresh.tick.get();
        async move {
            get_json::<Vec<EntryRow>>("/entries")
                .await
                .unwrap_or_default()
        }
    });
    let (selected_cat, set_selected_cat) = signal::<Option<String>>(None);
    let (selected_path, set_selected_path) = signal::<Option<String>>(None);
    let detail = LocalResource::new(move || {
        let path = selected_path.get();
        async move {
            match path {
                Some(p) => fetch_entry_detail(&p).await.ok(),
                None => None,
            }
        }
    });
    let detail_signal = Signal::derive(move || detail.get().flatten());

    // Rebuild only when `/entries` reloads — sort cycles reuse this Arc.
    let slim = Memo::new(move |_| {
        Arc::new(
            entries
                .get()
                .unwrap_or_default()
                .into_iter()
                .map(|r| SlimEntry {
                    path: r.path,
                    category: r.category,
                    size: r.size,
                    mtime_ns: r.mtime_ns,
                })
                .collect::<Vec<_>>(),
        )
    });

    let row_pairs = Memo::new(move |_| {
        slim.get()
            .iter()
            .map(|r| (r.path.clone(), r.category.clone()))
            .collect::<Vec<_>>()
    });

    let sort_ctx = ContentSortCtx::expect();

    let visible_cats = Signal::derive(move || {
        let cats = categories.get().unwrap_or_default();
        let rows = row_pairs.get();
        let q = search.trimmed.get();
        filter_categories(&cats, &rows, &q)
    });

    // Drop selection if filtered out.
    Effect::new(move |_| {
        let q = search.trimmed.get();
        let cats = visible_cats.get();
        if let Some(sel) = selected_cat.get_untracked()
            && !q.is_empty()
            && !cats.iter().any(|c| c == &sel)
        {
            set_selected_cat.set(None);
            set_selected_path.set(None);
        }
    });

    let paths = Memo::new(move |_| {
        let all = slim.get();
        let cat = selected_cat.get();
        let q = search.trimmed.get();
        let sort = sort_ctx.sort.get();
        // Active search keeps score ordering (same as current web filter); idle uses TUI content sort.
        if !q.trim().is_empty() {
            let pairs: Vec<(String, String)> = all
                .iter()
                .map(|r| (r.path.clone(), r.category.clone()))
                .collect();
            return path_rows(filter_snapshot_paths(&pairs, cat.as_deref(), &q));
        }
        let mut rows: Vec<(String, u64, Option<i64>)> = all
            .iter()
            .filter(|r| cat.as_ref().is_none_or(|c| &r.category == c))
            .map(|r| (r.path.clone(), r.size, r.mtime_ns))
            .collect();
        sort_snapshot_rows(&mut rows, sort);
        path_rows(rows.into_iter().map(|(p, _, _)| p))
    });

    let path_categories = Signal::derive(move || {
        slim.get()
            .iter()
            .map(|r| (r.path.clone(), r.category.clone()))
            .collect::<std::collections::HashMap<_, _>>()
    });

    Effect::new(move |_| {
        let list = paths.get();
        if let Some(sel) = selected_path.get_untracked()
            && !list.iter().any(|(_, k)| k == &sel)
        {
            set_selected_path.set(None);
        }
    });

    // Left pane: All + categories (empty string key = All).
    let nav = UiNav::expect();
    let cat_keys = Signal::derive(move || {
        let mut keys = vec![String::new()];
        keys.extend(visible_cats.get());
        keys
    });
    let (cat_nav, set_cat_nav) = signal(Some(selected_cat.get_untracked().unwrap_or_default()));
    Effect::new(move |_| {
        set_cat_nav.set(Some(selected_cat.get().unwrap_or_default()));
    });
    Effect::new(move |_| {
        let raw = cat_nav.get().unwrap_or_default();
        let next = if raw.is_empty() { None } else { Some(raw) };
        if next != selected_cat.get_untracked() {
            set_selected_cat.set(next);
            set_selected_path.set(None);
        }
    });
    install_list_nav(
        nav.left,
        string_list_nav(cat_keys, cat_nav.into(), set_cat_nav),
    );

    view! {
        <ThreePane
            left_title="Categories"
            middle_title="Contents"
            left=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    {move || {
                        let cats = visible_cats.get();
                        let _ = categories.get();
                        let _ = entries.get();
                        view! {
                            <ul class="panel-list">
                                <PanelRow
                                    label="All".to_string()
                                    selected=Signal::derive(move || selected_cat.get().is_none())
                                    on_select=Callback::new(move |_| {
                                        set_selected_cat.set(None);
                                        set_selected_path.set(None);
                                    })
                                />
                                {cats
                                    .into_iter()
                                    .map(|c| {
                                        let label = c.clone();
                                        let pick = c.clone();
                                        view! {
                                            <PanelRow
                                                label=label
                                                selected=Signal::derive({
                                                    let c = c.clone();
                                                    move || selected_cat.get().as_ref() == Some(&c)
                                                })
                                                on_select=Callback::new(move |_| {
                                                    set_selected_cat.set(Some(pick.clone()));
                                                    set_selected_path.set(None);
                                                })
                                            />
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }
                    }}
                </Suspense>
            }
            .into_any()
            middle=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    <PathsPane
                        main_mode=MainMode::Snapshot
                        paths=paths.into()
                        selected=selected_path.into()
                        on_select=Callback::new(move |p| set_selected_path.set(Some(p)))
                        path_categories=path_categories
                    />
                </Suspense>
            }
            .into_any()
            right=view! {
                <Suspense fallback=move || {
                    view! {
                        <div class="right-pane">
                            <div class="panel-titlebar">
                                <span class="tab-node tab-node--active tab-node--sm">"Viewer"</span>
                            </div>
                            <div class="panel-pad pane-empty">"…"</div>
                        </div>
                    }
                }>
                    <EntryRightPane detail=detail_signal/>
                </Suspense>
            }
            .into_any()
        />
    }
}
