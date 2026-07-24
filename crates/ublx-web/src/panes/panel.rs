//! Panel chrome, path lists, and entry/overview right panes.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::api::{EntryDetail, format_bytes, format_timestamp_ns};
use crate::focus::{PreviewKeysBus, UiNav, install_list_nav, string_list_nav};
use crate::kv_tables::KvTables;
use crate::multiselect::MultiselectCtx;
use crate::nav::MainMode;
use crate::search;
use crate::sort::ContentSortCtx;
use crate::space_menu::SpaceMenuCtx;
use crate::viewer::EntryViewer;
use crate::viewer_find::{ViewerFind, ViewerFindStrip, install_highlight_effect};

use super::RightTab;
use super::status::{PdfPageStatusNode, RightTabBtn, TextWindowStatusNode, TreeCollapseStatusNode};

#[component]
pub(super) fn PanelBox(
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
    /// Double-click → context / bulk menu (middle pane).
    #[prop(optional)]
    on_menu: Option<Callback<()>>,
    /// Ctrl/Cmd+click or check-column click → multi-select toggle.
    #[prop(optional)]
    on_toggle: Option<Callback<()>>,
    /// When set, reserve a check column (█ when true) for multi-select chrome.
    #[prop(optional)]
    checked: Option<Signal<bool>>,
    /// Extra class tokens (e.g. Delta `delta-added`).
    #[prop(optional)]
    class_extra: Option<&'static str>,
) -> impl IntoView {
    let show_check = checked.is_some();
    let checked_sig = checked.unwrap_or_else(|| Signal::derive(|| false));
    let row_ref = NodeRef::<leptos::html::Button>::new();

    // Arrow-key / click selection: keep the highlighted row in the scrollport.
    Effect::new(move |_| {
        if !selected.get() {
            return;
        }
        let Some(btn) = row_ref.get() else {
            return;
        };
        schedule_el_into_view(btn.into());
    });

    view! {
        <li>
            <button
                type="button"
                node_ref=row_ref
                class=move || {
                    let mut c = String::from("panel-row");
                    if selected.get() {
                        c.push_str(" panel-row--selected");
                    }
                    if show_check && checked_sig.get() {
                        c.push_str(" panel-row--checked");
                    }
                    if let Some(extra) = class_extra {
                        c.push(' ');
                        c.push_str(extra);
                    }
                    c
                }
                on:mousedown=move |ev| {
                    // Prevent the button from taking DOM focus; otherwise click→arrow
                    // leaves a stuck :focus highlight on the old row.
                    ev.prevent_default();
                }
                on:click=move |ev| {
                    if (ev.ctrl_key() || ev.meta_key())
                        && let Some(cb) = on_toggle
                    {
                        ev.prevent_default();
                        cb.run(());
                        return;
                    }
                    on_select.run(());
                }
                on:dblclick=move |ev| {
                    ev.prevent_default();
                    if let Some(cb) = on_menu {
                        cb.run(());
                    }
                }
            >
                <Show when=move || show_check>
                    <span
                        class="panel-row__check"
                        title="Toggle multi-select"
                        role="presentation"
                        on:click=move |ev| {
                            ev.stop_propagation();
                            ev.prevent_default();
                            if let Some(cb) = on_toggle {
                                cb.run(());
                            }
                        }
                        on:dblclick=move |ev| {
                            ev.stop_propagation();
                            ev.prevent_default();
                        }
                    >
                        {move || {
                            if checked_sig.get() {
                                crate::multiselect::CHECK_GLYPH
                            } else {
                                " "
                            }
                        }}
                    </span>
                </Show>
                <span class="panel-row__sym">{move || if selected.get() { "›" } else { " " }}</span>
                <span class="panel-row__text">{label.clone()}</span>
            </button>
        </li>
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

/// Scroll `el` into its nearest scrollport (`block`/`inline`: nearest).
pub(crate) fn schedule_el_into_view(el: web_sys::HtmlElement) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let cb = Closure::once_into_js(move || {
        let opts = ScrollIntoViewOptions::new();
        opts.set_block(ScrollLogicalPosition::Nearest);
        opts.set_inline(ScrollLogicalPosition::Nearest);
        el.scroll_into_view_with_scroll_into_view_options(&opts);
    });
    let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
}

/// After list reorder / selection change, keep the highlighted row on-screen.
pub(crate) fn schedule_scroll_selected_into_view(scroll: web_sys::HtmlElement) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let cb = Closure::once_into_js(move || {
        let Ok(Some(row)) =
            scroll.query_selector(".panel-row--selected, .settings-inline-row--selected")
        else {
            return;
        };
        let Ok(html) = row.dyn_into::<web_sys::HtmlElement>() else {
            return;
        };
        let opts = ScrollIntoViewOptions::new();
        opts.set_block(ScrollLogicalPosition::Nearest);
        opts.set_inline(ScrollLogicalPosition::Nearest);
        html.scroll_into_view_with_scroll_into_view_options(&opts);
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
    /// Optional path → catalog category (gates Open in new tab).
    #[prop(optional)]
    path_categories: Option<Signal<std::collections::HashMap<String, String>>>,
) -> impl IntoView {
    let search_q = Signal::derive(move || search::CatalogSearch::expect().trimmed.get());
    let sort_ctx = ContentSortCtx::expect();
    let nav = UiNav::expect();
    let multiselect = MultiselectCtx::expect();
    let space_menu = SpaceMenuCtx::expect();
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

    // Keep Space-menu + multi-select cursor in sync with the middle selection.
    Effect::new(move |_| {
        let path = selected.get();
        space_menu.middle_path.set(path.clone());
        let cat = match (&path, path_categories.as_ref()) {
            (Some(p), Some(cats)) => cats.get().get(p).cloned(),
            _ => None,
        };
        space_menu.middle_category.set(cat);
        if MultiselectCtx::applies(main_mode) {
            multiselect.cursor.set(path);
        }
    });

    let sort_label = Signal::derive(move || sort_ctx.sort.get().node_text(main_mode));
    let scroll_ref = NodeRef::<leptos::html::Div>::new();

    // Follow selection (arrows) + TUI `sort_anchor_path` when the list reorders.
    Effect::new(move |_| {
        let _ = sort_ctx.sort.get();
        let _ = paths.get();
        let _ = selected.get();
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
                                            let key_chk = key.clone();
                                            let pick_menu = key.clone();
                                            let pick_toggle = key.clone();
                                            let ms_applies = MultiselectCtx::applies(main_mode);
                                            let on_select_cb = Callback::new({
                                                let pick = pick.clone();
                                                move |_| on_select.run(pick.clone())
                                            });
                                            let on_menu_cb = Callback::new({
                                                let path_categories = path_categories;
                                                move |_| {
                                                    let path = pick_menu.clone();
                                                    on_select.run(path.clone());
                                                    if let Some(cats) = path_categories.as_ref() {
                                                        space_menu.middle_category.set(
                                                            cats.get_untracked().get(&path).cloned(),
                                                        );
                                                    }
                                                    let _ = space_menu
                                                        .open_from_middle_dblclick(main_mode, &path);
                                                }
                                            });
                                            if ms_applies {
                                                view! {
                                                    <PanelRow
                                                        label=label
                                                        selected=Signal::derive(move || {
                                                            selected.get().as_ref() == Some(&key_sel)
                                                        })
                                                        checked=Signal::derive(move || {
                                                            multiselect.is_checked(&key_chk)
                                                        })
                                                        on_select=on_select_cb
                                                        on_toggle=Callback::new(move |_| {
                                                            let path = pick_toggle.clone();
                                                            on_select.run(path.clone());
                                                            let _ = multiselect
                                                                .mouse_toggle_path(main_mode, &path);
                                                        })
                                                        on_menu=on_menu_cb
                                                    />
                                                }
                                                .into_any()
                                            } else {
                                                view! {
                                                    <PanelRow
                                                        label=label
                                                        selected=Signal::derive(move || {
                                                            selected.get().as_ref() == Some(&key_sel)
                                                        })
                                                        on_select=on_select_cb
                                                        on_menu=on_menu_cb
                                                    />
                                                }
                                                .into_any()
                                            }
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
                        let base = format_selection_counter(current, total);
                        let n = if MultiselectCtx::applies(main_mode) {
                            multiselect.count()
                        } else {
                            0
                        };
                        if n > 0 {
                            format!("{base} · {n} sel")
                        } else {
                            base
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
    let preview = PreviewKeysBus::expect();
    let find = ViewerFind::expect();
    let space_menu = SpaceMenuCtx::expect();
    install_highlight_effect(find, tab);

    // Prefer live detail category for Open-in-tab gating when it matches the
    // middle cursor (avoids stale detail while a new path is fetching).
    Effect::new(move |_| {
        let Some(d) = detail.get() else {
            return;
        };
        if space_menu.middle_path.get().as_deref() == Some(d.path.as_str()) {
            space_menu.middle_category.set(Some(d.category));
        }
    });

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
                                        <EntryViewer path=path category=category size=size/>
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
                            let viewer_tab = tab.get() == RightTab::Viewer;
                            let find_strip = find.strip_visible.get();
                            let tree_ctl = preview.tree.get().is_some();
                            // Size/mtime on Viewer; also keep bumper when a collapsible tree is up
                            // (Directory Viewer or Metadata schema) so Collapse all has a home.
                            if !viewer_tab && !find_strip && !tree_ctl {
                                return ().into_any();
                            }
                            let size_label = size_label.clone();
                            let mtime_label = mtime_label.clone();
                            let show_size_mtime = viewer_tab || tree_ctl;
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
                                        {if show_size_mtime {
                                            view! {
                                                <Show when=move || preview.pdf.get().is_some()>
                                                    <PdfPageStatusNode/>
                                                </Show>
                                                <Show when=move || preview.text_win.get().is_some()>
                                                    <TextWindowStatusNode/>
                                                </Show>
                                                <Show when=move || preview.tree.get().is_some()>
                                                    <TreeCollapseStatusNode/>
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
