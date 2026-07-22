//! App chrome: main tabs, project path, Last Snapshot footer.

use leptos::prelude::*;

use crate::api::{CatalogFlags, format_timestamp_ns};
use crate::modes::{DeltaMode, DuplicatesMode, LensesMode, SettingsMode, SnapshotMode};
use crate::nav::{MainMode, clamp_mode_to_visible, select_mode, use_main_mode};

#[component]
pub(crate) fn Shell(flags: CatalogFlags) -> impl IntoView {
    let flags = StoredValue::new(flags);
    let (mode, set_mode) = use_main_mode();

    // Deep-link may name a tab that is hidden for this catalog — fall back to Snapshot.
    Effect::new(move |_| {
        let f = flags.get_value();
        let clamped =
            clamp_mode_to_visible(mode.get(), f.has_lenses, f.has_delta, f.has_duplicates);
        if clamped != mode.get_untracked() {
            select_mode(set_mode, clamped);
        }
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
            <FooterNodes flags=flags/>
        </footer>
    }
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
fn FooterNodes(flags: StoredValue<CatalogFlags>) -> impl IntoView {
    view! {
        <div class="footer-nodes">
            <Show
                when=move || flags.get_value().last_snapshot_ns.is_some()
                fallback=|| ().into_any()
            >
                <span class="status-node">
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
                </span>
            </Show>
        </div>
    }
}
