//! UBLX embedded web UI (Leptos CSR + leptos-shadcn-ui).
//!
//! First pass: TUI chrome (main tabs, 3 panes, right-pane tabs, footer nodes).
//! Build with `./build.sh` / `mise run web`.

// Leptos `#[component]` fns are PascalCase by design (HTML-ish tags in `view!`).
#![allow(non_snake_case)]

mod api;
mod focus;
mod help;
mod keys;
mod modes;
mod nav;
mod panes;
mod search;
mod shell;
mod sort;
mod theme;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::api::{SettingsScope, fetch_settings, load_catalog_flags};
use crate::shell::Shell;
use crate::theme::{ThemeCssView, apply_theme_css};

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
                apply_theme_css(&ThemeCssView::from_parts(
                    v.css.name,
                    v.css.appearance,
                    v.css.vars,
                ));
            }
        });
    });

    view! {
        <div class="tui-shell">
            <Suspense fallback=move || {
                view! { <p class="shell-loading">"Connecting to ublx serve…"</p> }
            }>
                {move || match catalog.get() {
                    None => view! { <p class="shell-loading">"…"</p> }.into_any(),
                    Some(flags) => view! { <Shell flags=flags/> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}
