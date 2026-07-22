//! Duplicates mode: groups · member paths · entry detail.

use leptos::prelude::*;

use crate::api::{fetch_duplicates, fetch_entry_detail};
use crate::panes::{EntryRightPane, PanelRow, PathsPane, ThreePane};

#[component]
pub(crate) fn DuplicatesMode() -> impl IntoView {
    let catalog = LocalResource::new(fetch_duplicates);
    let (selected_id, set_selected_id) = signal::<Option<usize>>(None);
    let (selected_path, set_selected_path) = signal::<Option<String>>(None);

    Effect::new(move |_| {
        let groups = catalog.get().unwrap_or_default().groups;
        if selected_id.get_untracked().is_none()
            && let Some(first) = groups.first()
        {
            set_selected_id.set(Some(first.id));
        }
    });

    let paths = Signal::derive(move || {
        let cat = catalog.get().unwrap_or_default();
        let id = selected_id.get();
        cat.groups
            .into_iter()
            .find(|g| Some(g.id) == id)
            .map(|g| {
                g.paths
                    .into_iter()
                    .map(|p| (p.clone(), p))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });

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

    view! {
        <ThreePane
            left_title="Duplicate"
            middle_title="Paths"
            left=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    {move || {
                        let cat = catalog.get().unwrap_or_default();
                        if cat.groups.is_empty() {
                            return view! { <p class="pane-empty">"(no duplicates)"</p> }.into_any();
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
                                    {cat
                                        .groups
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
