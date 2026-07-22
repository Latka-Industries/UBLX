//! App chrome: main tabs, project path, Last Snapshot footer.

use leptos::prelude::*;
use leptos_router::components::{A, Route, Routes};
use leptos_router::hooks::use_location;
use leptos_router::path;

use crate::api::{format_timestamp_ns, CatalogFlags};
use crate::modes::{DeltaMode, DuplicatesMode, LensesMode, SettingsMode, SnapshotMode};

#[component]
pub(crate) fn Shell(flags: CatalogFlags) -> impl IntoView {
    let flags = StoredValue::new(flags);

    view! {
        <header class="main-chrome">
            <MainTabBar
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
            <Routes fallback=|| {
                view! { <p class="pane-empty">"Not found"</p> }
            }>
                <Route path=path!("/") view=SnapshotMode/>
                <Route path=path!("/snapshot") view=SnapshotMode/>
                <Route path=path!("/lenses") view=LensesMode/>
                <Route path=path!("/delta") view=DeltaMode/>
                <Route path=path!("/duplicates") view=DuplicatesMode/>
                <Route path=path!("/settings") view=SettingsMode/>
            </Routes>
        </main>

        <footer class="status-chrome">
            <FooterNodes flags=flags/>
        </footer>
    }
}

#[component]
fn MainTabBar(
    has_lenses: Signal<bool>,
    has_delta: Signal<bool>,
    has_duplicates: Signal<bool>,
) -> impl IntoView {
    view! {
        <nav class="main-tabs" aria-label="Main modes">
            <TabLink href="/" exact=true>"Snapshot"</TabLink>
            <Show when=move || has_lenses.get()>
                <TabLink href="/lenses">"Lenses"</TabLink>
            </Show>
            <Show when=move || has_delta.get()>
                <TabLink href="/delta">"Delta"</TabLink>
            </Show>
            <Show when=move || has_duplicates.get()>
                <TabLink href="/duplicates">"Duplicates"</TabLink>
            </Show>
            <TabLink href="/settings">"Settings"</TabLink>
        </nav>
    }
}

#[component]
fn TabLink(href: &'static str, #[prop(optional)] exact: bool, children: Children) -> impl IntoView {
    let location = use_location();
    let active = Signal::derive(move || {
        let path = location.pathname.get();
        if exact {
            path == "/" || path == "/snapshot"
        } else {
            path == href || path.starts_with(&format!("{href}/"))
        }
    });

    view! {
        <A href=href>
            <span
                class=move || {
                    if active.get() { "tab-node tab-node--active" } else { "tab-node" }
                }
            >
                {children()}
            </span>
        </A>
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
