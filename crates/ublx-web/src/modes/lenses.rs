//! Lenses mode: lens names · member paths · entry detail.

use leptos::prelude::*;

use crate::api::{fetch_entry_detail, fetch_lens_entries, fetch_lens_names};
use crate::panes::{EntryRightPane, PanelRow, PathsPane, ThreePane};

#[component]
pub(crate) fn LensesMode() -> impl IntoView {
    let lenses = LocalResource::new(fetch_lens_names);
    let (selected_lens, set_selected_lens) = signal::<Option<String>>(None);
    let (selected_path, set_selected_path) = signal::<Option<String>>(None);

    // Pick the first lens once names load (TUI lands on first left-row too).
    Effect::new(move |_| {
        let names = lenses.get().unwrap_or_default();
        if selected_lens.get_untracked().is_none()
            && let Some(first) = names.first()
        {
            set_selected_lens.set(Some(first.clone()));
        }
    });

    let members = LocalResource::new(move || {
        let name = selected_lens.get();
        async move {
            match name {
                Some(n) => fetch_lens_entries(&n).await,
                None => Vec::new(),
            }
        }
    });

    let paths = Signal::derive(move || {
        members
            .get()
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.path.clone(), r.path))
            .collect::<Vec<_>>()
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
            left_title="Lens"
            middle_title="Paths"
            left=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    {move || {
                        let names = lenses.get().unwrap_or_default();
                        if names.is_empty() {
                            return view! { <p class="pane-empty">"(no lenses)"</p> }.into_any();
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
