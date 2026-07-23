//! Overlay UI for Space / bulk menus.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use super::ctx::SpaceMenuCtx;
use super::helpers::sleep_ms;
use super::kinds::{Pending, pending_title};

#[component]
pub(crate) fn SpaceMenuPopup() -> impl IntoView {
    let menu = SpaceMenuCtx::expect();
    let panel_ref = NodeRef::<leptos::html::Div>::new();

    // Focus the panel when the menu opens so Enter / letters aren't lost to a leftover focus target.
    Effect::new(move |_| {
        if !menu.visible.get() {
            return;
        }
        let panel_ref = panel_ref;
        spawn_local(async move {
            sleep_ms(0).await;
            if let Some(el) = panel_ref.get_untracked() {
                let _ = el.focus();
            }
        });
    });

    view! {
        <Show when=move || menu.visible.get()>
            <div
                class="space-menu-overlay"
                role="dialog"
                aria-modal="true"
                aria-label="Quick actions"
                on:mousedown=move |ev| {
                    if let Some(t) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                        && t.class_list().contains("space-menu-overlay")
                    {
                        menu.close();
                    }
                }
            >
                <div
                    class="space-menu-panel"
                    node_ref=panel_ref
                    tabindex="-1"
                    on:mousedown=move |ev| ev.stop_propagation()
                    on:keydown=move |ev| handle_panel_keydown(menu, &ev)
                >
                    {move || pending_or_main(menu)}
                </div>
            </div>
        </Show>
    }
}

fn handle_panel_keydown(menu: SpaceMenuCtx, ev: &web_sys::KeyboardEvent) {
    // Text fields (rename / new lens / bulk rename) keep their own handlers.
    if let Some(t) = ev
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
    {
        let tag = t.tag_name().to_ascii_lowercase();
        if tag == "input" || tag == "textarea" {
            return;
        }
    }

    let key = ev.key();
    let ctrl = ev.ctrl_key() || ev.meta_key();
    let shift = ev.shift_key();
    if ctrl || shift {
        return;
    }

    if key == "Escape" {
        ev.prevent_default();
        ev.stop_propagation();
        menu.close();
        return;
    }
    if key == "Enter" || key == " " {
        ev.prevent_default();
        ev.stop_propagation();
        menu.submit_selected(None);
        return;
    }
    if key == "ArrowUp" {
        ev.prevent_default();
        ev.stop_propagation();
        menu.move_sel(-1);
        return;
    }
    if key == "ArrowDown" {
        ev.prevent_default();
        ev.stop_propagation();
        menu.move_sel(1);
        return;
    }
    if key.len() == 1 {
        let c = key
            .chars()
            .next()
            .map(|c| c.to_ascii_lowercase())
            .unwrap_or('\0');
        if c.is_ascii_alphabetic() || c.is_ascii_digit() {
            ev.prevent_default();
            ev.stop_propagation();
            if !menu.submit_hotkey(c, None) {
                match c {
                    'k' => menu.move_sel(-1),
                    'j' => menu.move_sel(1),
                    _ => {}
                }
            }
        }
    }
}

fn pending_or_main(menu: SpaceMenuCtx) -> AnyView {
    match menu.pending.get() {
        Some(Pending::Rename {
            target,
            draft,
            lens,
        }) => {
            let title = if lens {
                "Rename lens"
            } else {
                "Rename"
            };
            view! {
                <div class="space-menu-title">{format!(" {title} ")}</div>
                <p class="space-menu-hint">{target}</p>
                <input
                    class="space-menu-input"
                    type="text"
                    prop:value=draft
                    on:input=move |ev| {
                        menu.set_rename_draft(event_target_value(&ev));
                    }
                    on:keydown=move |ev| {
                        handle_enter_escape(&ev, menu, || menu.commit_rename_draft());
                    }
                />
                <p class="space-menu-hint">"Enter · Esc"</p>
            }
            .into_any()
        }
        Some(Pending::NewLens { draft, .. }) => view! {
            <div class="space-menu-title">" New lens "</div>
            <input
                class="space-menu-input"
                type="text"
                prop:value=draft
                on:input=move |ev| {
                    menu.set_new_lens_draft(event_target_value(&ev));
                }
                on:keydown=move |ev| {
                    handle_enter_escape(&ev, menu, || menu.commit_new_lens());
                }
            />
            <p class="space-menu-hint">"Enter · Esc"</p>
        }
        .into_any(),
        Some(Pending::BulkRename { draft, paths }) => view! {
            <div class="space-menu-title">" Bulk rename "</div>
            <p class="space-menu-hint">{format!("{} paths — one basename per line", paths.len())}</p>
            <textarea
                class="space-menu-textarea"
                prop:value=draft
                on:input=move |ev| {
                    menu.set_bulk_rename_draft(event_target_value(&ev));
                }
                on:keydown=move |ev| {
                    if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                        ev.prevent_default();
                        menu.commit_bulk_rename();
                    } else if ev.key() == "Escape" {
                        ev.prevent_default();
                        menu.close();
                    }
                }
            />
            <p class="space-menu-hint">"Ctrl+Enter · Esc"</p>
        }
        .into_any(),
        Some(Pending::DeleteConfirm { .. })
        | Some(Pending::EnhancePolicy { .. })
        | Some(Pending::LensPicker { .. })
        | None => {
            let kind = menu.kind.get();
            let title = pending_title(menu.pending.get().as_ref(), kind.as_ref());
            let rows = menu.display_rows();
            let sel = menu.selected.get();
            view! {
                <div class="space-menu-title">{title}</div>
                <ul class="space-menu-list">
                    {rows
                        .into_iter()
                        .enumerate()
                        .map(|(i, (text, _))| {
                            view! {
                                <li>
                                    <button
                                        type="button"
                                        class=move || {
                                            if i == sel {
                                                "space-menu-row is-selected"
                                            } else {
                                                "space-menu-row"
                                            }
                                        }
                                        on:mousedown=move |ev| {
                                            // Keep focus on the panel so letter/Enter keep working.
                                            ev.prevent_default();
                                        }
                                        on:click=move |_| menu.activate_index(i)
                                    >
                                        {text}
                                    </button>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
                <p class="space-menu-hint">"↑↓ · letter · Enter · click · Esc"</p>
            }
            .into_any()
        }
    }
}

fn handle_enter_escape(ev: &web_sys::KeyboardEvent, menu: SpaceMenuCtx, on_enter: impl FnOnce()) {
    if ev.key() == "Enter" {
        ev.prevent_default();
        on_enter();
    } else if ev.key() == "Escape" {
        ev.prevent_default();
        menu.close();
    }
}

fn event_target_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
        .or_else(|| {
            ev.target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                .map(|el| el.value())
        })
        .unwrap_or_default()
}
