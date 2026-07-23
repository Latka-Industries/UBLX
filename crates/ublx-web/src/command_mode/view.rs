//! Overlay UI for Command Mode + theme / root pickers.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

use super::ctx::{CommandModeCtx, Picker};
use super::helpers::sleep_ms;
use super::rows::{COMMAND_MODE_ROWS, popup_title};

#[component]
pub(crate) fn CommandModePopup() -> impl IntoView {
    let cmd = CommandModeCtx::expect();
    let panel_ref = NodeRef::<leptos::html::Div>::new();
    let was_open = StoredValue::new(false);

    // Focus the panel when the overlay opens (not on every selection change).
    Effect::new(move |_| {
        let open = cmd.menu_visible.get() || cmd.picker.with(|p| p.is_some());
        let prev = was_open.get_value();
        was_open.set_value(open);
        if !open || prev {
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

    // Keep the highlighted picker row in view while j/k / arrows move.
    Effect::new(move |_| {
        let Some(p) = cmd.picker.get() else {
            return;
        };
        let selected = match p {
            Picker::Theme { selected, .. } | Picker::Root { selected, .. } => selected,
        };
        spawn_local(async move {
            sleep_ms(0).await;
            scroll_selected_into_view(selected);
        });
    });

    view! {
        <Show when=move || cmd.toast.get().is_some()>
            <div class="command-mode-toast" role="status">{move || cmd.toast.get().unwrap_or_default()}</div>
        </Show>
        <Show when=move || cmd.menu_visible.get() || cmd.picker.get().is_some()>
            <div
                class="command-mode-overlay"
                role="dialog"
                aria-modal="true"
                aria-label="Command Mode"
                on:mousedown=move |ev| {
                    if let Some(t) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                        && t.class_list().contains("command-mode-overlay")
                    {
                        cmd.close_all();
                    }
                }
            >
                <div
                    class="command-mode-panel"
                    node_ref=panel_ref
                    tabindex="-1"
                    on:mousedown=move |ev| ev.stop_propagation()
                    on:keydown=move |ev| handle_panel_keydown(cmd, &ev)
                >
                    {move || panel_body(cmd)}
                </div>
            </div>
        </Show>
    }
}

fn scroll_selected_into_view(selected: usize) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(Some(el)) = doc.query_selector(&format!(
        ".command-mode-list [data-picker-idx=\"{selected}\"]"
    )) else {
        return;
    };
    let opts = web_sys::ScrollIntoViewOptions::new();
    opts.set_block(web_sys::ScrollLogicalPosition::Nearest);
    el.scroll_into_view_with_scroll_into_view_options(&opts);
}

fn handle_panel_keydown(cmd: CommandModeCtx, ev: &web_sys::KeyboardEvent) {
    if !cmd.picker_open() {
        return;
    }
    let key = ev.key();
    let code = ev.code();
    let ctrl = ev.ctrl_key() || ev.meta_key();
    let shift = ev.shift_key();
    if ctrl || shift {
        return;
    }

    let handled = match (key.as_str(), code.as_str()) {
        ("Escape", _) => {
            cmd.close_all();
            true
        }
        ("Enter" | " ", _) => {
            cmd.picker_submit();
            true
        }
        ("ArrowUp" | "k" | "K", _) | (_, "ArrowUp") => {
            cmd.picker_move(-1);
            true
        }
        ("ArrowDown" | "j" | "J", _) | (_, "ArrowDown") => {
            cmd.picker_move(1);
            true
        }
        _ => false,
    };
    if handled {
        ev.prevent_default();
        ev.stop_propagation();
    }
}

fn panel_body(cmd: CommandModeCtx) -> AnyView {
    if let Some(p) = cmd.picker.get() {
        return picker_view(cmd, p);
    }
    let leader = cmd.leader.get();
    let title = popup_title(leader);
    view! {
        <div class="command-mode-title">{title}</div>
        <table class="command-mode-table">
            <thead>
                <tr>
                    <th>"Key"</th>
                    <th>"Action"</th>
                </tr>
            </thead>
            <tbody>
                {COMMAND_MODE_ROWS
                    .iter()
                    .map(|(k, d)| {
                        let key = *k;
                        let desc = *d;
                        let ch = key.chars().next().unwrap_or('?');
                        view! {
                            <tr
                                class="command-mode-row"
                                on:click=move |_| {
                                    let _ = cmd.submit_hotkey(ch);
                                }
                            >
                                <td class="command-mode-key">{key}</td>
                                <td>{desc}</td>
                            </tr>
                        }
                    })
                    .collect_view()}
            </tbody>
        </table>
        <p class="command-mode-hint">"Type a letter · Esc cancel"</p>
    }
    .into_any()
}

fn picker_row_class(active: bool) -> &'static str {
    if active {
        "command-mode-row command-mode-row--sel"
    } else {
        "command-mode-row"
    }
}

fn set_picker_selected(cmd: CommandModeCtx, idx: usize) {
    cmd.picker.update(|p| match p.as_mut() {
        Some(Picker::Theme { selected, .. }) | Some(Picker::Root { selected, .. }) => {
            *selected = idx;
        }
        None => {}
    });
}

fn picker_shell(title: impl IntoView, hint: &'static str, items: impl IntoView) -> AnyView {
    view! {
        <div class="command-mode-title">{title}</div>
        <div class="command-mode-scroll">
            <ul class="command-mode-list" role="listbox">{items}</ul>
        </div>
        <p class="command-mode-hint">{hint}</p>
    }
    .into_any()
}

fn picker_view(cmd: CommandModeCtx, picker: Picker) -> AnyView {
    match picker {
        Picker::Theme {
            rows,
            selected,
            restore: _,
        } => {
            let mut theme_idx = 0usize;
            let items = rows
                .into_iter()
                .map(|row| match row {
                    crate::api::ThemePickerRow::Section { label } => view! {
                        <li class="command-mode-section" aria-hidden="true">
                            <span class="command-mode-section-rule">"──"</span>
                            <span class="command-mode-section-label">{label}</span>
                            <span class="command-mode-section-rule">"──"</span>
                        </li>
                    }
                    .into_any(),
                    crate::api::ThemePickerRow::Theme {
                        name,
                        appearance: _,
                        swatch,
                        css: _,
                    } => {
                        let i = theme_idx;
                        theme_idx += 1;
                        let active = i == selected;
                        let swatch_style = if swatch.is_empty() {
                            String::new()
                        } else {
                            format!("background: hsl({swatch}); color: hsl({swatch});")
                        };
                        view! {
                            <li>
                                <button
                                    type="button"
                                    data-picker-idx=i.to_string()
                                    class=picker_row_class(active)
                                    // Keep panel focus so arrows stay on the keybus / panel handler.
                                    on:mousedown=move |ev| ev.prevent_default()
                                    on:click=move |_| {
                                        set_picker_selected(cmd, i);
                                        cmd.picker_submit();
                                    }
                                >
                                    <span
                                        class="command-mode-swatch"
                                        style=swatch_style
                                        aria-hidden="true"
                                    >
                                        "█"
                                    </span>
                                    <span class="command-mode-theme-name">{name}</span>
                                </button>
                            </li>
                        }
                        .into_any()
                    }
                })
                .collect_view();
            picker_shell("Theme", "↑↓ / j k preview · Enter · Esc", items)
        }
        Picker::Root { paths, selected } => {
            let items = paths
                .into_iter()
                .enumerate()
                .map(|(i, path)| {
                    let active = i == selected;
                    let label = path.clone();
                    view! {
                        <li>
                            <button
                                type="button"
                                data-picker-idx=i.to_string()
                                class=picker_row_class(active)
                                title=label.clone()
                                on:mousedown=move |ev| ev.prevent_default()
                                on:click=move |_| {
                                    set_picker_selected(cmd, i);
                                    cmd.picker_submit();
                                }
                            >
                                {path}
                            </button>
                        </li>
                    }
                })
                .collect_view();
            picker_shell("Switch UBLX project", "↑↓ / j k · Enter · Esc", items)
        }
    }
}
