//! Bordered 3-pane layout (ratatui `Block`-style) and right-pane tabs.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::api::{EntryDetail, format_bytes, format_timestamp_ns};
use crate::focus::{PaneFocus, PreviewKeysBus, UiNav, install_list_nav, string_list_nav};
use crate::kv_tables::KvTables;
use crate::nav::MainMode;
use crate::search;
use crate::sort::ContentSortCtx;
use crate::viewer::EntryViewer;
use crate::viewer_find::{ViewerFind, ViewerFindStrip, install_highlight_effect};

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

/// Format `current/total` like TUI `middle::format_selection_counter` — both fields
/// share a width so the node does not jump when crossing 9→10 or 99→100.
fn format_selection_counter(current: usize, total: usize) -> String {
    let w = usize_digit_width(current)
        .max(usize_digit_width(total))
        .max(1);
    if current == 0 && total > 0 {
        // No selection yet — keep total width stable (em dash fills the current field).
        format!("{:>w$}/{total:>w$}", "—")
    } else {
        format!("{current:>w$}/{total:>w$}")
    }
}

fn usize_digit_width(n: usize) -> usize {
    if n == 0 { 1 } else { n.ilog10() as usize + 1 }
}

/// After sort reorders the list, keep the highlighted row on-screen (TUI `sort_anchor_path`).
fn scroll_selected_row_into_view(scroll: &web_sys::HtmlElement) {
    let Ok(Some(row)) = scroll.query_selector(".panel-row--selected") else {
        return;
    };
    let Ok(html) = row.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    let opts = ScrollIntoViewOptions::new();
    opts.set_block(ScrollLogicalPosition::Nearest);
    opts.set_inline(ScrollLogicalPosition::Nearest);
    html.scroll_into_view_with_scroll_into_view_options(&opts);
}

fn schedule_scroll_selected_into_view(scroll: web_sys::HtmlElement) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let cb = Closure::once_into_js(move || {
        scroll_selected_row_into_view(&scroll);
    });
    let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
}

/// Middle-pane path list with **right-aligned** sort (when TUI has it) + `current/total`
/// (TUI: `title_bottom` via [`src/render/panes/middle.rs`](../../../../src/render/panes/middle.rs)).
/// Used by Snapshot / Delta / Lenses / Duplicates.
/// Rows with an empty key render as non-selectable timestamp headers.
#[component]
pub(crate) fn PathsPane(
    /// Caller mode — drives sort node visibility + `s` / click cycle target.
    main_mode: MainMode,
    paths: Signal<Vec<(String, String)>>,
    selected: Signal<Option<String>>,
    on_select: Callback<String>,
) -> impl IntoView {
    let search_q = Signal::derive(move || search::CatalogSearch::expect().trimmed.get());
    let sort_ctx = ContentSortCtx::expect();
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

    let sort_label = Signal::derive(move || sort_ctx.sort.get().node_text(main_mode));
    let scroll_ref = NodeRef::<leptos::html::Div>::new();

    // TUI `sort_anchor_path`: selection stays on the same path; scroll the viewport to it.
    Effect::new(move |_| {
        let _ = sort_ctx.sort.get();
        let _ = paths.get();
        if selected.get_untracked().is_none() {
            return;
        }
        let Some(scroll) = scroll_ref.get() else {
            return;
        };
        schedule_scroll_selected_into_view(scroll.into());
    });

    view! {
        <div class="paths-pane">
            <div class="panel-scroll" node_ref=scroll_ref>
                <Show
                    when=move || paths.get().is_empty()
                    fallback=move || {
                        view! {
                            <ul class="panel-list">
                                <For
                                    each=move || paths.get()
                                    key=|(label, key)| {
                                        if key.is_empty() {
                                            // Delta timestamp headers share empty key — stabilize for For.
                                            format!("\0h:{label}")
                                        } else {
                                            key.clone()
                                        }
                                    }
                                    children=move |(label, key)| {
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
                                    }
                                />
                            </ul>
                        }
                        .into_any()
                    }
                >
                    <p class="pane-empty">
                        {move || {
                            search::empty_list_message(&search_q.get(), "(no contents)").to_string()
                        }}
                    </p>
                </Show>
            </div>
            <div class="pane-footer" aria-label="Sort and selection counter">
                <Show when=move || sort_label.get().is_some()>
                    <button
                        type="button"
                        class="status-node status-node--button"
                        title="Cycle content sort (s)"
                        on:click=move |_| sort_ctx.cycle(main_mode)
                    >
                        {move || sort_label.get().unwrap_or_default()}
                    </button>
                </Show>
                <span class="status-node status-node--counter">
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
                        format_selection_counter(current, total)
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
    let preview = PreviewKeysBus::expect();
    let find = ViewerFind::expect();
    install_highlight_effect(find, tab);

    // Remounted Metadata / Writing / Templates bodies need a find re-scan.
    Effect::new(move |_| {
        let _ = tab.get();
        let _ = detail.get();
        find.bump_content();
    });

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
                    let size = d.size;
                    let templates = d.templates.clone();
                    let metadata = d.metadata.clone();
                    let writing = d.writing.clone();
                    let metadata_tables = d.metadata_tables.clone();
                    let writing_tables = d.writing_tables.clone();
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
                                        <EntryViewer path=path category=category/>
                                    }
                                    .into_any()
                                }
                                RightTab::Templates => view! {
                                    <pre class="detail-pre">{templates.clone()}</pre>
                                }.into_any(),
                                RightTab::Metadata => {
                                    if !metadata_tables.is_empty() {
                                        view! { <KvTables sections=metadata_tables.clone()/> }
                                            .into_any()
                                    } else {
                                        view! {
                                            <pre class="detail-pre">
                                                {metadata.clone().unwrap_or_default()}
                                            </pre>
                                        }
                                        .into_any()
                                    }
                                }
                                RightTab::Writing => {
                                    if !writing_tables.is_empty() {
                                        view! { <KvTables sections=writing_tables.clone()/> }
                                            .into_any()
                                    } else {
                                        view! {
                                            <pre class="detail-pre">
                                                {writing.clone().unwrap_or_default()}
                                            </pre>
                                        }
                                        .into_any()
                                    }
                                }
                            }}
                        </div>
                        {move || {
                            let viewer_meta = tab.get() == RightTab::Viewer;
                            let find_strip = find.strip_visible.get();
                            if !viewer_meta && !find_strip {
                                return ().into_any();
                            }
                            let size_label = size_label.clone();
                            let mtime_label = mtime_label.clone();
                            view! {
                                <div class="right-pane-footer" aria-label="Viewer status">
                                    <div class="right-pane-footer__start">
                                        {if find_strip {
                                            view! { <ViewerFindStrip/> }.into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </div>
                                    <div class="right-pane-footer__end">
                                        {if viewer_meta {
                                            view! {
                                                <Show when=move || preview.pdf.get().is_some()>
                                                    <PdfPageStatusNode/>
                                                </Show>
                                                <span class="status-node">{size_label}</span>
                                                {if show_mtime {
                                                    view! {
                                                        <span class="status-node">{mtime_label}</span>
                                                    }
                                                    .into_any()
                                                } else {
                                                    ().into_any()
                                                }}
                                            }
                                            .into_any()
                                        } else {
                                            ().into_any()
                                        }}
                                    </div>
                                </div>
                            }
                            .into_any()
                        }}
                    }
                    .into_any()
                }}
            </Show>
        </div>
    }
}

#[component]
fn PdfPageStatusNode() -> impl IntoView {
    let preview = PreviewKeysBus::expect();
    let (editing, set_editing) = signal(false);
    let (draft, set_draft) = signal(String::new());
    let input_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        if !editing.get() {
            return;
        }
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
            el.select();
        }
    });

    let start_edit = move |_| {
        let Some(ctl) = preview.pdf.get_untracked() else {
            return;
        };
        set_draft.set(ctl.page.get_untracked().max(1).to_string());
        set_editing.set(true);
    };

    let commit = move || {
        let Some(ctl) = preview.pdf.get_untracked() else {
            set_editing.set(false);
            return;
        };
        let raw = draft.get_untracked();
        let trimmed = raw.trim();
        if !trimmed.is_empty()
            && let Ok(n) = trimmed.parse::<u32>()
        {
            ctl.goto.run(n);
        }
        set_editing.set(false);
    };

    let cancel = move || {
        set_editing.set(false);
    };

    view! {
        <span
            class=move || {
                if editing.get() {
                    "status-node status-node--page status-node--page-editing"
                } else {
                    "status-node status-node--button status-node--page"
                }
            }
            title="Click to jump to page"
            on:click=move |ev| {
                if editing.get_untracked() {
                    return;
                }
                ev.prevent_default();
                start_edit(());
            }
        >
            "Page "
            <Show
                when=move || editing.get()
                fallback=move || {
                    view! {
                        <span class="status-node__page-num">
                            {move || {
                                preview
                                    .pdf
                                    .get()
                                    .map(|c| c.page.get().max(1))
                                    .unwrap_or(1)
                            }}
                        </span>
                    }
                }
            >
                <input
                    node_ref=input_ref
                    class="status-node__page-input"
                    type="text"
                    inputmode="numeric"
                    pattern="[0-9]*"
                    prop:value=move || draft.get()
                    size=move || {
                        let digits = preview
                            .pdf
                            .get()
                            .and_then(|c| c.page_count.get())
                            .unwrap_or_else(|| {
                                preview.pdf.get().map(|c| c.page.get()).unwrap_or(1)
                            })
                            .to_string()
                            .len();
                        digits.clamp(2, 6) as u32
                    }
                    aria-label="Page number"
                    on:input=move |ev| {
                        set_draft.set(event_target_value(&ev));
                    }
                    on:keydown=move |ev| {
                        ev.stop_propagation();
                        match ev.key().as_str() {
                            "Enter" => {
                                ev.prevent_default();
                                commit();
                            }
                            "Escape" => {
                                ev.prevent_default();
                                cancel();
                            }
                            _ => {}
                        }
                    }
                    on:blur=move |_| commit()
                    on:click=move |ev| ev.stop_propagation()
                />
            </Show>
            {move || {
                preview.pdf.get().and_then(|c| c.page_count.get()).map(|n| {
                    view! { <span>{format!(" / {n}")}</span> }.into_any()
                })
            }}
        </span>
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
