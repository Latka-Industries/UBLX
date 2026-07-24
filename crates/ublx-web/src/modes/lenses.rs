//! Lenses mode: lens names · member paths · entry detail.

use leptos::prelude::*;

use crate::api::{fetch_entry_detail_opt, fetch_lens_entries, fetch_lens_names};
use crate::catalog_refresh::CatalogRefresh;
use crate::focus::{UiNav, install_list_nav, string_list_nav};
use crate::nav::MainMode;
use crate::panes::{EntryRightPane, PanelRow, PathsPane, ThreePane};
use crate::search::{CatalogSearch, empty_list_message, filter_labels, filter_paths, path_rows};
use crate::space_menu::SpaceMenuCtx;

#[component]
pub(crate) fn LensesMode() -> impl IntoView {
    let search = CatalogSearch::expect();
    let space_menu = SpaceMenuCtx::expect();
    let refresh = CatalogRefresh::expect();
    let lenses = LocalResource::new(move || {
        let _ = refresh.tick.get();
        async move { fetch_lens_names().await }
    });
    let (selected_lens, set_selected_lens) = signal::<Option<String>>(None);
    let (selected_path, set_selected_path) = signal::<Option<String>>(None);

    let visible_lenses = Signal::derive(move || {
        let names = lenses.get().unwrap_or_default();
        let q = search.trimmed.get();
        filter_labels(&names, &q)
    });

    // Pick the first lens once names load (TUI lands on first left-row too).
    Effect::new(move |_| {
        let names = visible_lenses.get();
        if selected_lens.get_untracked().is_none()
            && let Some(first) = names.first()
        {
            set_selected_lens.set(Some(first.clone()));
        }
        if let Some(sel) = selected_lens.get_untracked()
            && !names.iter().any(|n| n == &sel)
        {
            set_selected_lens.set(names.first().cloned());
            set_selected_path.set(None);
        }
    });

    let members = LocalResource::new(move || {
        let _ = refresh.tick.get();
        let name = selected_lens.get();
        async move {
            match name {
                Some(n) => fetch_lens_entries(&n).await,
                None => Vec::new(),
            }
        }
    });

    let paths = Signal::derive(move || {
        let q = search.trimmed.get();
        let raw: Vec<String> = members
            .get()
            .unwrap_or_default()
            .into_iter()
            .map(|r| r.path)
            .collect();
        path_rows(filter_paths(&raw, &q))
    });

    let path_categories = Signal::derive(move || {
        members
            .get()
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.path, r.category))
            .collect::<std::collections::HashMap<_, _>>()
    });

    let detail = LocalResource::new(move || {
        let path = selected_path.get();
        async move { fetch_entry_detail_opt(path).await }
    });
    let detail_signal = Signal::derive(move || detail.get().flatten());

    let nav = UiNav::expect();
    let (lens_nav, set_lens_nav) = signal(selected_lens.get_untracked());
    Effect::new(move |_| {
        set_lens_nav.set(selected_lens.get());
    });
    Effect::new(move |_| {
        space_menu.left_label.set(selected_lens.get());
    });
    Effect::new(move |_| {
        let b = lens_nav.get();
        if b != selected_lens.get_untracked() {
            set_selected_lens.set(b);
            set_selected_path.set(None);
        }
    });
    install_list_nav(
        nav.left,
        string_list_nav(visible_lenses, lens_nav.into(), set_lens_nav),
    );

    view! {
        <ThreePane
            left_title="Lens"
            middle_title="Paths"
            left=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    {move || {
                        let _ = lenses.get();
                        let names = visible_lenses.get();
                        if names.is_empty() {
                            let empty = empty_list_message(&search.trimmed.get(), "(no lenses)");
                            return view! { <p class="pane-empty">{empty}</p> }.into_any();
                        }
                        view! {
                            <ul class="panel-list">
                                {names
                                    .into_iter()
                                    .map(|name| {
                                        let label = name.clone();
                                        let pick = name.clone();
                                        view! {
                                            <PanelRow
                                                label=label
                                                selected=Signal::derive({
                                                    let name = name.clone();
                                                    move || {
                                                        selected_lens.get().as_ref() == Some(&name)
                                                    }
                                                })
                                                on_select=Callback::new(move |_| {
                                                    set_selected_lens.set(Some(pick.clone()));
                                                    set_selected_path.set(None);
                                                })
                                            />
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }
                        .into_any()
                    }}
                </Suspense>
            }
            .into_any()
            middle=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    <PathsPane
                        main_mode=MainMode::Lenses
                        paths=paths
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
