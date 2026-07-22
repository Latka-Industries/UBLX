//! Bordered 3-pane layout (ratatui `Block`-style) and right-pane tabs.

use leptos::prelude::*;

use crate::api::{EntryDetail, format_bytes, format_timestamp_ns};
use crate::focus::{PaneFocus, UiNav, install_list_nav, string_list_nav};
use crate::search;

/// Shared 3-pane TUI layout — bordered boxes with title nodes.
/// Pane focus lives in [`UiNav`] (keyboard + click).
#[component]
pub(crate) fn ThreePane(
    left_title: &'static str,
    middle_title: &'static str,
    left: AnyView,
    middle: AnyView,
    right: AnyView,
) -> impl IntoView {
    let nav = UiNav::expect();
    let focus = nav.pane;
    let set_focus = nav.set_pane;

    view! {
        <div class="three-pane">
            <PanelBox
                title=left_title
                focused=Signal::derive(move || focus.get() == PaneFocus::Left)
                on_focus=Callback::new(move |_| set_focus.set(PaneFocus::Left))
            >
                {left}
            </PanelBox>
            <PanelBox
                title=middle_title
                focused=Signal::derive(move || focus.get() == PaneFocus::Middle)
                on_focus=Callback::new(move |_| set_focus.set(PaneFocus::Middle))
            >
                {middle}
            </PanelBox>
            <PanelBox
                title="Right"
                hide_default_title=true
                focused=Signal::derive(|| false)
                on_focus=Callback::new(|_| {})
            >
                {right}
            </PanelBox>
        </div>
    }
}

#[component]
fn PanelBox(
    title: &'static str,
    focused: Signal<bool>,
    on_focus: Callback<()>,
    #[prop(optional)] hide_default_title: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <section
            class=move || {
                if focused.get() {
                    "panel panel--focused"
                } else {
                    "panel"
                }
            }
            on:mousedown=move |_| on_focus.run(())
        >
            <div class="panel-frame">
                <Show when=move || !hide_default_title>
                    <div class="panel-titlebar">
                        <span class=move || {
                            if focused.get() {
                                "tab-node tab-node--active tab-node--sm"
                            } else {
                                "tab-node tab-node--sm"
                            }
                        }>{title}</span>
                    </div>
                </Show>
                <div class="panel-inner">{children()}</div>
            </div>
        </section>
    }
}

#[component]
pub(crate) fn PanelRow(
    label: String,
    selected: Signal<bool>,
    on_select: Callback<()>,
) -> impl IntoView {
    view! {
        <li>
            <button
                type="button"
                class=move || {
                    if selected.get() {
                        "panel-row panel-row--selected"
                    } else {
                        "panel-row"
                    }
                }
                on:mousedown=move |ev| {
                    // Prevent the button from taking DOM focus; otherwise click→arrow
                    // leaves a stuck :focus highlight on the old row.
                    ev.prevent_default();
                }
                on:click=move |_| on_select.run(())
            >
                <span class="panel-row__sym">{move || if selected.get() { "›" } else { " " }}</span>
                <span class="panel-row__text">{label.clone()}</span>
            </button>
        </li>
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RightTab {
    Viewer,
    Templates,
    Metadata,
    Writing,
}

impl RightTab {
    fn label(self) -> &'static str {
        match self {
            Self::Viewer => "Viewer",
            Self::Templates => "Templates",
            Self::Metadata => "Metadata",
            Self::Writing => "Writing",
        }
    }
}

/// Snapshot-overview text in the Delta right pane.
#[component]
pub(crate) fn OverviewRightPane(text: Signal<String>) -> impl IntoView {
    view! {
        <div class="right-pane">
            <div class="panel-titlebar">
                <span class="tab-node tab-node--active tab-node--sm">"Snapshot overview"</span>
            </div>
            <div class="panel-pad">
                <Show
                    when=move || !text.get().trim().is_empty()
                    fallback=|| view! { <p class="pane-empty">"(no delta history)"</p> }
                >
                    <pre class="detail-pre">{move || text.get()}</pre>
                </Show>
            </div>
        </div>
    }
}

/// Middle-pane path list with **right-aligned** `current/total` footer node
/// (TUI: `title_bottom` via [`src/render/panes/middle.rs`](../../../../src/render/panes/middle.rs)).
/// Used by Snapshot / Delta / Lenses / Duplicates.
/// Rows with an empty key render as non-selectable timestamp headers.
#[component]
pub(crate) fn PathsPane(
    paths: Signal<Vec<(String, String)>>,
    selected: Signal<Option<String>>,
    on_select: Callback<String>,
) -> impl IntoView {
    let search_q = Signal::derive(move || search::CatalogSearch::expect().trimmed.get());
    let nav = UiNav::expect();
    let keys = Signal::derive(move || {
        paths
            .get()
            .into_iter()
            .filter_map(|(_, k)| (!k.is_empty()).then_some(k))
            .collect::<Vec<_>>()
    });
    let (bridge, set_bridge) = signal(selected.get_untracked());
    Effect::new(move |_| {
        set_bridge.set(selected.get());
    });
    Effect::new(move |_| {
        let b = bridge.get();
        if b != selected.get_untracked()
            && let Some(p) = b
        {
            on_select.run(p);
        }
    });
    install_list_nav(nav.middle, string_list_nav(keys, bridge.into(), set_bridge));

    view! {
        <div class="paths-pane">
            <div class="panel-scroll">
                {move || {
                    let rows = paths.get();
                    if rows.is_empty() {
                        let empty = search::empty_list_message(
                            &search_q.get(),
                            "(no contents)",
                        );
                        return view! { <p class="pane-empty">{empty}</p> }.into_any();
                    }
                    view! {
                        <ul class="panel-list">
                            {rows
                                .into_iter()
                                .map(|(label, key)| {
                                    if key.is_empty() {
                                        view! {
                                            <li class="panel-heading">{label}</li>
                                        }
                                        .into_any()
                                    } else {
                                        let pick = key.clone();
                                        let key_sel = key.clone();
                                        view! {
                                            <PanelRow
                                                label=label
                                                selected=Signal::derive(move || {
                                                    selected.get().as_ref() == Some(&key_sel)
                                                })
                                                on_select=Callback::new({
                                                    let pick = pick.clone();
                                                    move |_| on_select.run(pick.clone())
                                                })
                                            />
                                        }
                                        .into_any()
                                    }
                                })
                                .collect_view()}
                        </ul>
                    }
                    .into_any()
                }}
            </div>
            <div class="pane-footer" aria-label="Selection counter">
                <span class="status-node">
                    {move || {
                        let rows = paths.get();
                        let selectable: Vec<_> =
                            rows.iter().filter(|(_, k)| !k.is_empty()).collect();
                        let total = selectable.len();
                        let current = selected
                            .get()
                            .and_then(|s| selectable.iter().position(|(_, k)| **k == s))
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        if total == 0 {
                            "0/0".to_string()
                        } else if current == 0 {
                            format!("—/{total}")
                        } else {
                            format!("{current}/{total}")
                        }
                    }}
                </span>
            </div>
        </div>
    }
}

/// Snapshot / Lenses / Duplicates right pane: Viewer + Zahir section tabs.
#[component]
pub(crate) fn EntryRightPane(detail: Signal<Option<EntryDetail>>) -> impl IntoView {
    let (tab, set_tab) = signal(RightTab::Viewer);
    let tabs = crate::focus::RightTabBus::expect();

    let available = move |t: RightTab, d: &Option<EntryDetail>| match t {
        RightTab::Viewer => true,
        RightTab::Templates => d.as_ref().is_some_and(EntryDetail::has_templates),
        RightTab::Metadata => d.as_ref().is_some_and(EntryDetail::has_metadata),
        RightTab::Writing => d.as_ref().is_some_and(EntryDetail::has_writing),
    };

    Effect::new(move |_| {
        let d = detail.get();
        let cur = tab.get_untracked();
        if !available(cur, &d) {
            set_tab.set(RightTab::Viewer);
        }
    });

    // Hotkeys: v / t / m / w
    Effect::new(move |_| {
        if let Some(req) = tabs.request.get() {
            let d = detail.get_untracked();
            if available(req, &d) {
                set_tab.set(req);
            }
            tabs.set_request.set(None);
        }
    });

    // Shift+Tab cycles visible right tabs.
    Effect::new(move |_| {
        let tick = tabs.cycle_tick.get();
        if tick == 0 {
            return;
        }
        let d = detail.get_untracked();
        let order = [
            RightTab::Viewer,
            RightTab::Templates,
            RightTab::Metadata,
            RightTab::Writing,
        ];
        let visible: Vec<_> = order.into_iter().filter(|t| available(*t, &d)).collect();
        if visible.is_empty() {
            return;
        }
        let cur = tab.get_untracked();
        let idx = visible.iter().position(|t| *t == cur).unwrap_or(0);
        set_tab.set(visible[(idx + 1) % visible.len()]);
    });

    view! {
        <div class="right-pane">
            <Show
                when=move || detail.get().is_some()
                fallback=|| {
                    view! {
                        <div class="panel-titlebar">
                            <span class="tab-node tab-node--active tab-node--sm">"Viewer"</span>
                        </div>
                        <div class="panel-pad pane-empty">"Select a path in Contents"</div>
                    }
                }
            >
                {move || {
                    let d = detail.get().unwrap_or_default();
                    let path = d.path.clone();
                    let category = d.category.clone();
                    let show_category = !category.is_empty();
                    let size = d.size;
                    let templates = d.templates.clone();
                    let metadata = d.metadata.clone();
                    let writing = d.writing.clone();
                    let show_templates = d.has_templates();
                    let show_metadata = d.has_metadata();
                    let show_writing = d.has_writing();
                    let size_label = format_bytes(size);
                    let mtime_label = d.mtime_ns.map(format_timestamp_ns).unwrap_or_default();
                    let show_mtime = !mtime_label.is_empty();

                    view! {
                        <div class="panel-titlebar right-pane-chrome">
                            <nav class="right-tabs" aria-label="Right pane">
                                <RightTabBtn tab=RightTab::Viewer current=tab set=set_tab/>
                                <Show when=move || show_templates>
                                    <RightTabBtn tab=RightTab::Templates current=tab set=set_tab/>
                                </Show>
                                <Show when=move || show_metadata>
                                    <RightTabBtn tab=RightTab::Metadata current=tab set=set_tab/>
                                </Show>
                                <Show when=move || show_writing>
                                    <RightTabBtn tab=RightTab::Writing current=tab set=set_tab/>
                                </Show>
                            </nav>
                        </div>
                        <div class="panel-pad">
                            {move || match tab.get() {
                                RightTab::Viewer => {
                                    let path = path.clone();
                                    let category = category.clone();
                                    view! {
                                        <div class="entry-viewer">
                                            <p class="entry-viewer__path">{path}</p>
                                            <Show when=move || show_category>
                                                <p class="entry-viewer__meta">{category.clone()}</p>
                                            </Show>
                                            <p class="pane-empty entry-viewer__note">
                                                "(viewer — disk file preview not available over serve yet)"
                                            </p>
                                        </div>
                                    }
                                    .into_any()
                                }
                                RightTab::Templates => view! {
                                    <pre class="detail-pre">{templates.clone()}</pre>
                                }.into_any(),
                                RightTab::Metadata => view! {
                                    <pre class="detail-pre">{metadata.clone().unwrap_or_default()}</pre>
                                }.into_any(),
                                RightTab::Writing => view! {
                                    <pre class="detail-pre">{writing.clone().unwrap_or_default()}</pre>
                                }.into_any(),
                            }}
                        </div>
                        <Show when=move || tab.get() == RightTab::Viewer>
                            <div class="right-pane-footer" aria-label="Size and modified time">
                                <span class="status-node">{size_label.clone()}</span>
                                {if show_mtime {
                                    view! {
                                        <span class="status-node">{mtime_label.clone()}</span>
                                    }
                                    .into_any()
                                } else {
                                    ().into_any()
                                }}
                            </div>
                        </Show>
                    }
                    .into_any()
                }}
            </Show>
        </div>
    }
}

#[component]
fn RightTabBtn(
    tab: RightTab,
    current: ReadSignal<RightTab>,
    set: WriteSignal<RightTab>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || {
                if current.get() == tab {
                    "tab-node tab-node--active tab-node--sm"
                } else {
                    "tab-node tab-node--sm"
                }
            }
            on:click=move |_| set.set(tab)
        >
            {tab.label()}
        </button>
    }
}
