//! App chrome: main tabs, project path, catalog search / Last Snapshot footer.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

use crate::api::{CatalogFlags, format_timestamp_ns};
use crate::focus::{PaneFocus, RightTabBus, UiNav};
use crate::keys::{WebAction, action_from_keydown, typing_in_form_field};
use crate::modes::{DeltaMode, DuplicatesMode, LensesMode, SettingsMode, SnapshotMode};
use crate::nav::{MainMode, clamp_mode_to_visible, select_mode, use_main_mode};
use crate::search::{CatalogSearch, SEARCH_LABEL};

#[component]
pub(crate) fn Shell(flags: CatalogFlags) -> impl IntoView {
    let flags = StoredValue::new(flags);
    let (mode, set_mode) = use_main_mode();
    let search = CatalogSearch::provide();
    let (nav, tabs) = UiNav::provide();

    // Deep-link may name a tab that is hidden for this catalog — fall back to Snapshot.
    Effect::new(move |_| {
        let f = flags.get_value();
        let clamped =
            clamp_mode_to_visible(mode.get(), f.has_lenses, f.has_delta, f.has_duplicates);
        if clamped != mode.get_untracked() {
            select_mode(set_mode, clamped);
        }
    });

    // Global keybus — TUI-shaped hotkeys (ignore while search strip is active or typing in forms).
    Effect::new(move |_| {
        let Some(window) = web_sys::window() else {
            return;
        };
        let search = search;
        let nav = nav;
        let tabs = tabs;
        let mode = mode;
        let set_mode = set_mode;
        let flags = flags;
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |ev: KeyboardEvent| {
            if typing_in_form_field() {
                return;
            }
            let search_active = search.active.get_untracked();
            let Some(action) = action_from_keydown(&ev, search_active) else {
                return;
            };
            ev.prevent_default();
            dispatch_action(action, search, nav, tabs, mode, set_mode, flags);
        }) as Box<dyn FnMut(_)>);
        let _ =
            window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
        // Keep listener for the shell lifetime (CSR single mount).
        closure.forget();
    });

    view! {
        <header class="main-chrome">
            <MainTabBar
                mode=mode
                set_mode=set_mode
                has_lenses=Signal::derive(move || flags.get_value().has_lenses)
                has_delta=Signal::derive(move || flags.get_value().has_delta)
                has_duplicates=Signal::derive(move || flags.get_value().has_duplicates)
            />
            <div class="brand" aria-label="UBLX">"UBLX"</div>
        </header>

        <div class="project-path" title=move || flags.get_value().root.clone().unwrap_or_default()>
            {
                move || {
                    flags
                        .get_value()
                        .root
                        .clone()
                        .unwrap_or_else(|| "—".into())
                }
            }
        </div>

        <main class="mode-body">
            {move || match mode.get() {
                MainMode::Snapshot => view! { <SnapshotMode/> }.into_any(),
                MainMode::Lenses => view! { <LensesMode/> }.into_any(),
                MainMode::Delta => view! { <DeltaMode/> }.into_any(),
                MainMode::Duplicates => view! { <DuplicatesMode/> }.into_any(),
                MainMode::Settings => view! { <SettingsMode/> }.into_any(),
            }}
        </main>

        <footer class="status-chrome">
            <FooterNodes flags=flags search=search/>
        </footer>
    }
}

fn dispatch_action(
    action: WebAction,
    search: CatalogSearch,
    nav: UiNav,
    tabs: RightTabBus,
    mode: ReadSignal<MainMode>,
    set_mode: WriteSignal<MainMode>,
    flags: StoredValue<CatalogFlags>,
) {
    let f = flags.get_value();
    match action {
        WebAction::SearchStart => search.start(),
        WebAction::MainMode(m) => {
            if m.is_visible(f.has_lenses, f.has_delta, f.has_duplicates) {
                select_mode(set_mode, m);
            }
        }
        WebAction::MainModeToggle => {
            let next = next_visible_mode(
                mode.get_untracked(),
                f.has_lenses,
                f.has_delta,
                f.has_duplicates,
            );
            select_mode(set_mode, next);
        }
        WebAction::FocusLeft => nav.set_pane.set(PaneFocus::Left),
        WebAction::FocusMiddle => nav.set_pane.set(PaneFocus::Middle),
        WebAction::FocusCycle => {
            let next = nav.pane.get_untracked().cycle();
            nav.set_pane.set(next);
        }
        WebAction::MoveUp => {
            if let Some(list) = nav.active_list() {
                list.move_by.run(-1);
            }
        }
        WebAction::MoveDown => {
            if let Some(list) = nav.active_list() {
                list.move_by.run(1);
            }
        }
        WebAction::MoveUpFast => {
            if let Some(list) = nav.active_list() {
                list.move_by.run(-10);
            }
        }
        WebAction::MoveDownFast => {
            if let Some(list) = nav.active_list() {
                list.move_by.run(10);
            }
        }
        WebAction::ListTop => {
            if let Some(list) = nav.active_list() {
                list.to_start.run(());
            }
        }
        WebAction::ListBottom => {
            if let Some(list) = nav.active_list() {
                list.to_end.run(());
            }
        }
        WebAction::RightTab(t) => {
            tabs.set_request.set(Some(t));
            nav.set_pane.set(PaneFocus::Right);
        }
        WebAction::CycleRightTab => {
            tabs.bump_cycle.update(|n| *n = n.wrapping_add(1));
            nav.set_pane.set(PaneFocus::Right);
        }
    }
}

fn next_visible_mode(
    current: MainMode,
    has_lenses: bool,
    has_delta: bool,
    has_duplicates: bool,
) -> MainMode {
    let visible: Vec<MainMode> = MainMode::ALL
        .into_iter()
        .filter(|m| m.is_visible(has_lenses, has_delta, has_duplicates))
        .collect();
    if visible.is_empty() {
        return MainMode::Snapshot;
    }
    let idx = visible.iter().position(|m| *m == current).unwrap_or(0);
    visible[(idx + 1) % visible.len()]
}

#[component]
fn MainTabBar(
    mode: ReadSignal<MainMode>,
    set_mode: WriteSignal<MainMode>,
    has_lenses: Signal<bool>,
    has_delta: Signal<bool>,
    has_duplicates: Signal<bool>,
) -> impl IntoView {
    view! {
        <nav class="main-tabs" aria-label="Main modes">
            {MainMode::ALL
                .into_iter()
                .map(|m| {
                    let visible = Signal::derive(move || {
                        m.is_visible(has_lenses.get(), has_delta.get(), has_duplicates.get())
                    });
                    view! {
                        <Show when=move || visible.get()>
                            <TabBtn
                                label=m.label()
                                active=Signal::derive(move || mode.get() == m)
                                on_click=Callback::new(move |_| select_mode(set_mode, m))
                            />
                        </Show>
                    }
                })
                .collect_view()}
        </nav>
    }
}

#[component]
fn TabBtn(label: &'static str, active: Signal<bool>, on_click: Callback<()>) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || {
                if active.get() {
                    "tab-node tab-node--active"
                } else {
                    "tab-node"
                }
            }
            on:click=move |_| on_click.run(())
        >
            {label}
        </button>
    }
}

#[component]
fn FooterNodes(flags: StoredValue<CatalogFlags>, search: CatalogSearch) -> impl IntoView {
    let strip = search.strip_visible;
    let input_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        if search.active.get()
            && let Some(el) = input_ref.get()
        {
            let _ = el.focus();
        }
    });

    view! {
        <div class="footer-nodes">
            <Show
                when=move || strip.get()
                fallback=move || {
                    view! {
                        <Show
                            when=move || flags.get_value().last_snapshot_ns.is_some()
                            fallback=|| ().into_any()
                        >
                            <button
                                type="button"
                                class="status-node status-node--button"
                                title="Open catalog search (/)"
                                on:click=move |_| search.start()
                            >
                                {
                                    move || {
                                        flags
                                            .get_value()
                                            .last_snapshot_ns
                                            .map(format_timestamp_ns)
                                            .map(|t| format!("Last Snapshot: {t}"))
                                            .unwrap_or_default()
                                    }
                                }
                            </button>
                        </Show>
                    }
                    .into_any()
                }
            >
                <div
                    class=move || {
                        if search.active.get() {
                            "catalog-search catalog-search--active"
                        } else {
                            "catalog-search"
                        }
                    }
                    on:click=move |_| search.start()
                >
                    <span class="catalog-search__label">{SEARCH_LABEL}</span>
                    <Show
                        when=move || search.active.get()
                        fallback=move || {
                            view! {
                                <span class="catalog-search__query">
                                    {move || search.query.get()}
                                </span>
                            }
                            .into_any()
                        }
                    >
                        <input
                            node_ref=input_ref
                            type="text"
                            class="catalog-search__input"
                            prop:value=move || search.query.get()
                            on:input=move |ev| {
                                search.set_query.set(event_target_value(&ev));
                            }
                            on:keydown=move |ev| {
                                let key = ev.key();
                                if key == "Escape" {
                                    ev.prevent_default();
                                    search.clear();
                                } else if key == "Enter" {
                                    ev.prevent_default();
                                    search.submit();
                                }
                            }
                        />
                    </Show>
                </div>
            </Show>
        </div>
    }
}
