//! App chrome: main tabs, project path, catalog search / Last Snapshot footer.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

use crate::api::{CatalogFlags, format_timestamp_ns};
use crate::modes::{DeltaMode, DuplicatesMode, LensesMode, SettingsMode, SnapshotMode};
use crate::nav::{MainMode, clamp_mode_to_visible, select_mode, use_main_mode};
use crate::search::{CatalogSearch, SEARCH_LABEL};

#[component]
pub(crate) fn Shell(flags: CatalogFlags) -> impl IntoView {
    let flags = StoredValue::new(flags);
    let (mode, set_mode) = use_main_mode();
    let search = CatalogSearch::provide();

    // Deep-link may name a tab that is hidden for this catalog — fall back to Snapshot.
    Effect::new(move |_| {
        let f = flags.get_value();
        let clamped =
            clamp_mode_to_visible(mode.get(), f.has_lenses, f.has_delta, f.has_duplicates);
        if clamped != mode.get_untracked() {
            select_mode(set_mode, clamped);
        }
    });

    // TUI `/` opens catalog search (ignore when already typing in an input/textarea/select).
    Effect::new(move |_| {
        let Some(window) = web_sys::window() else {
            return;
        };
        let search = search;
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |ev: KeyboardEvent| {
            if search.active.get_untracked() {
                return;
            }
            if ev.key() != "/" || ev.ctrl_key() || ev.meta_key() || ev.alt_key() {
                return;
            }
            if typing_in_form_field() {
                return;
            }
            ev.prevent_default();
            search.start();
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

fn typing_in_form_field() -> bool {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    let Some(el) = doc.active_element() else {
        return false;
    };
    let tag = el.tag_name().to_ascii_lowercase();
    matches!(tag.as_str(), "input" | "textarea" | "select")
        || el.get_attribute("contenteditable").is_some()
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
