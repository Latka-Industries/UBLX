//! Right-pane status bumper nodes and tab buttons.

use leptos::prelude::*;

use crate::api::format_bytes;
use crate::focus::PreviewKeysBus;

use super::RightTab;

#[component]
pub(super) fn PdfPageStatusNode() -> impl IntoView {
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
pub(super) fn TextWindowStatusNode() -> impl IntoView {
    let preview = PreviewKeysBus::expect();
    view! {
        <span class="status-node status-node--window" title="Byte window (Shift+J/K)">
            {move || {
                let Some(ctl) = preview.text_win.get() else {
                    return String::new();
                };
                let off = ctl.offset.get();
                let len = ctl.byte_len.get();
                let tot = ctl.total.get();
                let end = if len == 0 {
                    off
                } else {
                    off.saturating_add(len - 1)
                };
                format!(
                    "{}–{} / {}",
                    format_bytes(off),
                    format_bytes(end),
                    format_bytes(tot)
                )
            }}
        </span>
    }
}

#[component]
pub(super) fn TreeCollapseStatusNode() -> impl IntoView {
    let preview = PreviewKeysBus::expect();
    view! {
        <button
            type="button"
            class="status-node status-node--button"
            title="Expand all tree nodes"
            prop:disabled=move || {
                preview
                    .tree
                    .get()
                    .is_none_or(|c| !c.can_expand.get())
            }
            on:click=move |ev| {
                ev.prevent_default();
                if let Some(ctl) = preview.tree.get_untracked() {
                    ctl.expand_all.run(());
                }
            }
        >
            "Expand all"
        </button>
        <button
            type="button"
            class="status-node status-node--button"
            title="Collapse all tree nodes"
            prop:disabled=move || {
                preview
                    .tree
                    .get()
                    .is_none_or(|c| !c.can_collapse.get())
            }
            on:click=move |ev| {
                ev.prevent_default();
                if let Some(ctl) = preview.tree.get_untracked() {
                    ctl.collapse_all.run(());
                }
            }
        >
            "Collapse all"
        </button>
    }
}

#[component]
pub(super) fn RightTabBtn(
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
