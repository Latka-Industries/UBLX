//! Snapshot mode: categories · contents · right pane.

use leptos::prelude::*;

use crate::api::{EntryRow, fetch_entry_detail, get_json};
use crate::panes::{EntryRightPane, PanelRow, PathsPane, ThreePane};

#[component]
pub(crate) fn SnapshotMode() -> impl IntoView {
    let categories = LocalResource::new(|| async move {
        get_json::<Vec<String>>("/categories")
            .await
            .unwrap_or_default()
    });
    let (selected_cat, set_selected_cat) = signal::<Option<String>>(None);
    let entries = LocalResource::new(move || {
        let cat = selected_cat.get();
        async move {
            let url = match cat.as_deref() {
                None | Some("") => "/entries".to_string(),
                Some(c) => format!("/entries?category={}", urlencoding::encode(c)),
            };
            get_json::<Vec<EntryRow>>(&url).await.unwrap_or_default()
        }
    });
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

    let paths = Signal::derive(move || {
        entries
            .get()
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.path.clone(), r.path))
            .collect::<Vec<_>>()
    });

    view! {
        <ThreePane
            left_title="Categories"
            middle_title="Contents"
            left=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    {move || {
                        let cats = categories.get().unwrap_or_default();
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
