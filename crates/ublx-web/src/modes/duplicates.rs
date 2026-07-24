//! Duplicates mode: groups · member paths · entry detail.

use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::*;

use crate::api::{EntryRow, fetch_duplicates, fetch_entry_detail_opt, get_json};
use crate::catalog_refresh::CatalogRefresh;
use crate::focus::{UiNav, id_list_nav, install_list_nav};
use crate::nav::MainMode;
use crate::panes::{EntryRightPane, PanelRow, PathsPane, ThreePane};
use crate::search::{
    CatalogSearch, empty_list_message, filter_paths, fuzzy_matches_field, path_rows,
};
use crate::sort::{ContentSortCtx, sort_snapshot_rows};
use crate::space_menu::SpaceMenuCtx;

#[component]
pub(crate) fn DuplicatesMode() -> impl IntoView {
    let search = CatalogSearch::expect();
    let space_menu = SpaceMenuCtx::expect();
    let refresh = CatalogRefresh::expect();
    let catalog = LocalResource::new(move || {
        let _ = refresh.tick.get();
        async move { fetch_duplicates().await }
    });
    // Size / Mod sort needs catalog sizes — same `/entries` payload Snapshot uses.
    let entries = LocalResource::new(move || {
        let _ = refresh.tick.get();
        async move {
            get_json::<Vec<EntryRow>>("/entries")
                .await
                .unwrap_or_default()
        }
    });
    let (selected_id, set_selected_id) = signal::<Option<usize>>(None);
    let (selected_path, set_selected_path) = signal::<Option<String>>(None);
    let sort_ctx = ContentSortCtx::expect();

    let entry_meta = Memo::new(move |_| {
        Arc::new(
            entries
                .get()
                .unwrap_or_default()
                .into_iter()
                .map(|e| (e.path, (e.size, e.mtime_ns)))
                .collect::<HashMap<_, _>>(),
        )
    });

    let path_categories = Signal::derive(move || {
        entries
            .get()
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.path, e.category))
            .collect::<HashMap<_, _>>()
    });

    let visible_groups = Signal::derive(move || {
        let cat = catalog.get().unwrap_or_default();
        let q = search.trimmed.get();
        if q.is_empty() {
            return cat.groups;
        }
        cat.groups
            .into_iter()
            .filter(|g| {
                fuzzy_matches_field(&g.label, &q)
                    || g.paths.iter().any(|p| fuzzy_matches_field(p, &q))
            })
            .collect()
    });

    Effect::new(move |_| {
        let groups = visible_groups.get();
        if selected_id.get_untracked().is_none()
            && let Some(first) = groups.first()
        {
            set_selected_id.set(Some(first.id));
        }
        if let Some(id) = selected_id.get_untracked()
            && !groups.iter().any(|g| g.id == id)
        {
            set_selected_id.set(groups.first().map(|g| g.id));
            set_selected_path.set(None);
        }
    });

    let paths = Memo::new(move |_| {
        let groups = visible_groups.get();
        let id = selected_id.get();
        let q = search.trimmed.get();
        let sort = sort_ctx.sort.get();
        let ignored = space_menu.ignored.get();
        let raw = groups
            .into_iter()
            .find(|g| Some(g.id) == id)
            .map(|g| g.paths)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| !ignored.contains(p))
            .collect::<Vec<_>>();
        let filtered = filter_paths(&raw, &q);
        if !q.trim().is_empty() {
            return path_rows(filtered);
        }
        let meta = entry_meta.get();
        let mut rows: Vec<(String, u64, Option<i64>)> = filtered
            .into_iter()
            .map(|p| {
                let (size, mtime) = meta.get(&p).copied().unwrap_or((0, None));
                (p, size, mtime)
            })
            .collect();
        sort_snapshot_rows(&mut rows, sort);
        path_rows(rows.into_iter().map(|(p, _, _)| p))
    });

    let detail = LocalResource::new(move || {
        let path = selected_path.get();
        async move { fetch_entry_detail_opt(path).await }
    });
    let detail_signal = Signal::derive(move || detail.get().flatten());

    let nav = UiNav::expect();
    let group_ids = Signal::derive(move || {
        visible_groups
            .get()
            .into_iter()
            .map(|g| g.id)
            .collect::<Vec<_>>()
    });
    let (id_nav, set_id_nav) = signal(selected_id.get_untracked());
    Effect::new(move |_| {
        set_id_nav.set(selected_id.get());
    });
    Effect::new(move |_| {
        let b = id_nav.get();
        if b != selected_id.get_untracked() {
            set_selected_id.set(b);
            set_selected_path.set(None);
        }
    });
    install_list_nav(nav.left, id_list_nav(group_ids, id_nav.into(), set_id_nav));

    view! {
        <ThreePane
            left_title="Duplicate"
            middle_title="Paths"
            left=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    {move || {
                        let cat = catalog.get().unwrap_or_default();
                        let groups = visible_groups.get();
                        if groups.is_empty() {
                            let empty =
                                empty_list_message(&search.trimmed.get(), "(no duplicates)");
                            return view! { <p class="pane-empty">{empty}</p> }.into_any();
                        }
                        let mode_hint = match cat.mode.as_str() {
                            "hash" => " (H)",
                            "name_size" => " (N/S)",
                            _ => "",
                        };
                        view! {
                            <div class="panel-scroll">
                                <p class="pane-empty" style="padding-bottom: 0.2rem;">
                                    {format!("Grouping{mode_hint}")}
                                </p>
                                <ul class="panel-list">
                                    {groups
                                        .into_iter()
                                        .map(|g| {
                                            let id = g.id;
                                            let label = g.label;
                                            view! {
                                                <PanelRow
                                                    label=label
                                                    selected=Signal::derive(move || {
                                                        selected_id.get() == Some(id)
                                                    })
                                                    on_select=Callback::new(move |_| {
                                                        set_selected_id.set(Some(id));
                                                        set_selected_path.set(None);
                                                    })
                                                />
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            </div>
                        }
                        .into_any()
                    }}
                </Suspense>
            }
            .into_any()
            middle=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    <PathsPane
                        main_mode=MainMode::Duplicates
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
