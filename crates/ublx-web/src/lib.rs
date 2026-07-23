//! UBLX embedded web UI (Leptos CSR + leptos-shadcn-ui).
//!
//! First pass: TUI chrome (main tabs, 3 panes, right-pane tabs, footer nodes).
//! Build with `./build.sh` / `mise run web`.

// Leptos `#[component]` fns are PascalCase by design (HTML-ish tags in `view!`).
#![allow(non_snake_case)]

mod api;
mod catalog_refresh;
mod command_mode;
mod focus;
mod help;
mod keys;
mod kv_tables;
mod modes;
mod multiselect;
mod nav;
mod panes;
mod search;
mod shell;
mod sort;
mod space_menu;
mod theme;
mod viewer;
mod viewer_find;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::api::{SettingsScope, fetch_settings, load_catalog_flags};
use crate::shell::Shell;
use crate::theme::apply_theme_css_body;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        view! { <App/> }
    });
}

#[component]
fn App() -> impl IntoView {
    let catalog = LocalResource::new(load_catalog_flags);

    // Bootstrap live CSS tokens from effective (merged local) settings.
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(v) = fetch_settings(SettingsScope::Local).await {
                apply_theme_css_body(&v.css);
            }
        });
    });

    view! {
        <Suspense fallback=move || {
            view! {
                <div class="shell-boot">
                    <p class="shell-loading">"Connecting to UBLX…"</p>
                </div>
            }
        }>
            {move || match catalog.get() {
                None => {
                    view! {
                        <div class="shell-boot">
                            <p class="shell-loading">"…"</p>
                        </div>
                    }
                    .into_any()
                }
                Some(flags) => view! {
                    <div class="tui-shell">
                        <Shell flags=flags/>
                    </div>
                }
                .into_any(),
            }}
        </Suspense>
    }
}
