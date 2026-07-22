//! Snapshot mode: categories · contents · right pane.

use leptos::prelude::*;

use crate::api::{get_json, EntryRow};
use crate::panes::{PanelRow, RightPaneShell, ThreePane};

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
                    {move || {
                        let rows = entries.get().unwrap_or_default();
                        if rows.is_empty() {
                            return view! { <p class="pane-empty">"(no contents)"</p> }.into_any();
                        }
                        view! {
                            <ul class="panel-list">
                                {rows
                                    .into_iter()
                                    .map(|r| {
                                        let path = r.path.clone();
                                        let pick = r.path.clone();
                                        view! {
                                            <PanelRow
                                                label=path.clone()
                                                selected=Signal::derive({
                                                    let path = path.clone();
                                                    move || selected_path.get().as_ref() == Some(&path)
                                                })
                                                on_select=Callback::new(move |_| {
                                                    set_selected_path.set(Some(pick.clone()));
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
            right=view! { <RightPaneShell/> }.into_any()
        />
    }
}
