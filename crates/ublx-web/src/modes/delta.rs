//! Delta mode: Added / Modified / Removed · paths · Snapshot overview.

use leptos::prelude::*;

use crate::api::{DeltaKind, DeltaRow, fetch_delta_catalog, format_timestamp_ns};
use crate::focus::{ListNav, UiNav, install_list_nav};
use crate::nav::MainMode;
use crate::panes::{OverviewRightPane, PanelRow, PathsPane, ThreePane};
use crate::search::{CatalogSearch, fuzzy_matches_field};
use crate::sort::{ContentSortCtx, sort_delta_rows};

#[component]
pub(crate) fn DeltaMode() -> impl IntoView {
    let catalog = LocalResource::new(fetch_delta_catalog);
    let (kind, set_kind) = signal(DeltaKind::Added);
    let (selected_path, set_selected_path) = signal::<Option<String>>(None);

    let overview = Signal::derive(move || catalog.get().unwrap_or_default().overview_text());

    let search = CatalogSearch::expect();
    let sort_ctx = ContentSortCtx::expect();
    let paths = Signal::derive(move || {
        let cat = catalog.get().unwrap_or_default();
        let rows = cat.rows_for(kind.get());
        let q = search.trimmed.get();
        let kept: Vec<&DeltaRow> = if q.is_empty() {
            rows
        } else {
            rows.into_iter()
                .filter(|r| fuzzy_matches_field(&r.path, &q))
                .collect()
        };
        let mut timed: Vec<(i64, String)> = kept
            .into_iter()
            .map(|r| (r.created_ns, r.path.clone()))
            .collect();
        // When searching, keep encounter order from filter; idle applies Time ↕ like TUI.
        if q.trim().is_empty() {
            sort_delta_rows(&mut timed, sort_ctx.sort.get());
        }
        display_paths_timed(&timed)
    });

    let nav = UiNav::expect();
    install_list_nav(
        nav.left,
        ListNav {
            move_by: Callback::new(move |delta: i32| {
                let all = DeltaKind::ALL;
                let idx = all
                    .iter()
                    .position(|k| *k == kind.get_untracked())
                    .unwrap_or(0);
                let n = all.len() as i32;
                let next = ((idx as i32 + delta).clamp(0, n - 1)) as usize;
                if all[next] != kind.get_untracked() {
                    set_kind.set(all[next]);
                    set_selected_path.set(None);
                }
            }),
            to_start: Callback::new(move |_| {
                set_kind.set(DeltaKind::ALL[0]);
                set_selected_path.set(None);
            }),
            to_end: Callback::new(move |_| {
                set_kind.set(*DeltaKind::ALL.last().unwrap());
                set_selected_path.set(None);
            }),
        },
    );

    view! {
        <ThreePane
            left_title="Delta type"
            middle_title="Paths"
            left=view! {
                <Suspense fallback=move || view! { <p class="pane-empty">"…"</p> }>
                    {move || {
                        let _ = catalog.get();
                        view! {
                            <ul class="panel-list">
                                {DeltaKind::ALL
                                    .into_iter()
                                    .map(|k| {
                                        view! {
                                            <PanelRow
                                                label=k.label().to_string()
                                                class_extra=k.css_class()
                                                selected=Signal::derive(move || kind.get() == k)
                                                on_select=Callback::new(move |_| {
                                                    set_kind.set(k);
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
                        main_mode=MainMode::Delta
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
                                <span class="tab-node tab-node--active tab-node--sm">
                                    "Snapshot overview"
                                </span>
                            </div>
                            <div class="panel-pad pane-empty">"…"</div>
                        </div>
                    }
                }>
                    <OverviewRightPane text=overview/>
                </Suspense>
            }
            .into_any()
        />
    }
}

/// Path rows for the middle pane: timestamp headers + indented paths (TUI-shaped).
/// Keys for path rows are `created_ns\0path`; header rows use an empty key.
fn display_paths_timed(rows: &[(i64, String)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut current_ns: Option<i64> = None;
    for (ns, path) in rows {
        if current_ns != Some(*ns) {
            current_ns = Some(*ns);
            out.push((format_timestamp_ns(*ns), String::new()));
        }
        out.push((format!("  {path}"), format!("{ns}\0{path}")));
    }
    out
}
