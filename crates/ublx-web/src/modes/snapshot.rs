//! Snapshot mode: categories · contents · right pane (+ server windowed list).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::fetch_entry_detail_opt;
use crate::catalog_data::CatalogData;
use crate::catalog_refresh::{CatalogRefresh, CatalogScope};
use crate::entries_window::{ENTRIES_PAGE_SIZE, EntriesWindow};
use crate::focus::{UiNav, index_list_nav, install_list_nav, string_list_nav};
use crate::nav::MainMode;
use crate::panes::{EntryRightPane, PanelRow, PathsPane, ThreePane};
use crate::search::{CatalogSearch, filter_labels, path_rows};
use crate::sort::{ContentSortCtx, sort_snapshot_rows};
use crate::util::sleep_ms;

#[component]
pub(crate) fn SnapshotMode() -> impl IntoView {
    let search = CatalogSearch::expect();
    let catalog = CatalogData::expect();
    let refresh = CatalogRefresh::expect();
    let entries = EntriesWindow::expect();
    let categories = catalog.categories;
    let (selected_cat, set_selected_cat) = signal::<Option<String>>(None);
    let (selected_path, set_selected_path) = signal::<Option<String>>(None);
    let (selected_idx, set_selected_idx) = signal::<Option<usize>>(None);
    let (visible_range, set_visible_range) = signal((0usize, 0usize));
    let contents_ready = entries.ready();

    let clear_path_sel = move || {
        set_selected_path.set(None);
        set_selected_idx.set(None);
    };

    // Root / catalog refresh: drop UI selection (EntriesWindow already cleared its filter).
    Effect::new(move |_| {
        let _ = refresh.tick(CatalogScope::ENTRIES);
        set_selected_cat.set(None);
        clear_path_sel();
        set_visible_range.set((0, 0));
    });

    let detail = LocalResource::new(move || {
        let path = selected_path.get();
        async move { fetch_entry_detail_opt(path).await }
    });
    let detail_signal = Signal::derive(move || detail.get().flatten());

    Effect::new(move |_| {
        let cat = selected_cat.get();
        let prev = entries.filter_category();
        if prev != cat {
            entries.set_category(cat);
            clear_path_sel();
        }
    });

    Effect::new(move |_| {
        let q = search.trimmed.get();
        if q.is_empty() {
            entries.set_contains(String::new());
            return;
        }
        spawn_local(async move {
            let expected = q.clone();
            sleep_ms(EntriesWindow::search_debounce_ms()).await;
            if search.trimmed.get_untracked() == expected {
                entries.set_contains(expected);
            }
        });
    });

    Effect::new(move |_| {
        let total = entries.total().get();
        if let Some(i) = selected_idx.get_untracked()
            && i >= total
        {
            clear_path_sel();
        }
    });

    // Windowed fetch around visible range + selection.
    Effect::new(move |_| {
        if entries.is_dense().get() {
            return;
        }
        let (start, end) = visible_range.get();
        let _ = contents_ready.get();
        let _ = entries.cache_revision().get();
        entries.ensure_range(start, end);
        if let Some(i) = selected_idx.get() {
            entries.ensure_range(i.saturating_sub(32), i.saturating_add(33));
            if let Some(p) = entries.path_at(i)
                && selected_path.get_untracked().as_deref() != Some(p.as_str())
            {
                set_selected_path.set(Some(p));
            }
        }
    });

    let sort_ctx = ContentSortCtx::expect();

    let dense_paths = Memo::new(move |_| {
        let Some(all) = entries.dense_rows().get() else {
            return Vec::new();
        };
        let sort = sort_ctx.sort.get();
        let q = search.trimmed.get();
        let mut rows: Vec<(String, u64, Option<i64>)> = all
            .iter()
            .map(|r| (r.path.clone(), r.size, r.mtime_ns))
            .collect();
        if q.is_empty() {
            sort_snapshot_rows(&mut rows, sort);
        }
        path_rows(rows.into_iter().map(|(p, _, _)| p))
    });

    let window_paths = Memo::new(move |_| {
        let (mut start, mut end) = visible_range.get();
        let total = entries.total().get();
        let _ = entries.cache_revision().get();
        // Before PathsPane measures scroll, visible_range is (0,0) — still paint a first page
        // of placeholders / rows so we never keep the previous root's list on screen.
        if end <= start && total > 0 {
            start = 0;
            end = ENTRIES_PAGE_SIZE.min(total);
        }
        entries.window_rows(start, end)
    });

    let path_categories = Signal::derive(move || {
        if entries.is_dense().get() {
            entries.dense_path_categories()
        } else if let Some(p) = selected_path.get() {
            entries
                .category_of_path(&p)
                .map(|c| std::collections::HashMap::from([(p, c)]))
                .unwrap_or_default()
        } else {
            std::collections::HashMap::new()
        }
    });

    let list_total = entries.total();

    let visible_cats = Signal::derive(move || {
        let cats = categories.get().unwrap_or_default();
        let q = search.trimmed.get();
        filter_labels(&cats, &q)
    });

    Effect::new(move |_| {
        let q = search.trimmed.get();
        let cats = visible_cats.get();
        if let Some(sel) = selected_cat.get_untracked()
            && !q.is_empty()
            && !cats.iter().any(|c| c == &sel)
        {
            set_selected_cat.set(None);
            clear_path_sel();
        }
    });

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
            clear_path_sel();
        }
    });
    install_list_nav(
        nav.left,
        string_list_nav(cat_keys, cat_nav.into(), set_cat_nav),
    );

    let on_select_path = Callback::new(move |p: String| {
        if let Some(i) = entries.index_of_path(&p) {
            set_selected_idx.set(Some(i));
        }
        set_selected_path.set(Some(p));
    });

    // Dense: PathsPane owns string nav. Windowed: index nav over server total.
    Effect::new(move |_| {
        if !entries.is_dense().get() {
            nav.middle.set(Some(index_list_nav(
                list_total,
                selected_idx.into(),
                set_selected_idx,
            )));
        }
    });
    on_cleanup(move || {
        nav.middle.set(None);
    });

    let clear_all_sel = Callback::new(move |_: ()| {
        set_selected_cat.set(None);
        clear_path_sel();
    });
    let pick_cat = Callback::new(move |c: String| {
        set_selected_cat.set(Some(c));
        clear_path_sel();
    });

    view! {
        <ThreePane
            left_title="Categories"
            middle_title="Contents"
            left=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"Loading..."</p> }>
                    {move || {
                        let cats = visible_cats.get();
                        let _ = contents_ready.get();
                        let _ = categories.get();
                        view! {
                            <ul class="panel-list">
                                <PanelRow
                                    label="All".to_string()
                                    selected=Signal::derive(move || selected_cat.get().is_none())
                                    on_select=Callback::new(move |_| clear_all_sel.run(()))
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
                                                    pick_cat.run(pick.clone())
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
                <Show
                    when=move || contents_ready.get()
                    fallback=move || view! { <p class="pane-empty">"Loading..."</p> }
                >
                    // Key on catalog generation so Dense→Windowed root switches never reuse
                    // the previous PathsPane DOM / scroll state.
                    {move || {
                        let _gen = entries.list_generation().get();
                        if entries.is_dense().get() {
                            view! {
                                <PathsPane
                                    main_mode=MainMode::Snapshot
                                    paths=dense_paths.into()
                                    selected=selected_path.into()
                                    on_select=on_select_path
                                    path_categories=path_categories
                                    list_total=list_total
                                />
                            }
                            .into_any()
                        } else {
                            view! {
                                <PathsPane
                                    main_mode=MainMode::Snapshot
                                    paths=window_paths.into()
                                    selected=selected_path.into()
                                    on_select=on_select_path
                                    path_categories=path_categories
                                    list_total=list_total
                                    selected_index=selected_idx.into()
                                    on_visible_range=Callback::new(move |r| set_visible_range.set(r))
                                    paths_are_window=true
                                />
                            }
                            .into_any()
                        }
                    }}
                </Show>
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
