//! Snapshot mode: categories · contents · right pane (+ catalog search filter).

use leptos::prelude::*;

use crate::api::{EntryRow, fetch_entry_detail, get_json};
use crate::panes::{EntryRightPane, PanelRow, PathsPane, ThreePane};
use crate::search::{CatalogSearch, filter_categories, filter_snapshot_paths, path_rows};

#[component]
pub(crate) fn SnapshotMode() -> impl IntoView {
    let search = CatalogSearch::expect();
    let categories = LocalResource::new(|| async move {
        get_json::<Vec<String>>("/categories")
            .await
            .unwrap_or_default()
    });
    // Full catalog once — needed so category visibility can see matches in other categories.
    let entries = LocalResource::new(|| async move {
        get_json::<Vec<EntryRow>>("/entries")
            .await
            .unwrap_or_default()
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

    let row_pairs = Signal::derive(move || {
        entries
            .get()
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.path, r.category))
            .collect::<Vec<_>>()
    });

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

    let paths = Signal::derive(move || {
        let rows = row_pairs.get();
        let cat = selected_cat.get();
        let q = search.trimmed.get();
        path_rows(filter_snapshot_paths(&rows, cat.as_deref(), &q))
    });

    Effect::new(move |_| {
        let list = paths.get();
        if let Some(sel) = selected_path.get_untracked()
            && !list.iter().any(|(_, k)| k == &sel)
        {
            set_selected_path.set(None);
        }
    });

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
                        paths=paths
                        selected=selected_path.into()
                        on_select=Callback::new(move |p| set_selected_path.set(Some(p)))
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
