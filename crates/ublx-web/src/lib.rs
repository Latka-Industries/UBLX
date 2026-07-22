//! UBLX embedded web UI (Leptos CSR + leptos-shadcn-ui).
//!
//! First pass: TUI chrome (main tabs, 3 panes, right-pane tabs, footer nodes).
//! Build with `./build.sh` / `mise run web`.

// Leptos `#[component]` fns are PascalCase by design (HTML-ish tags in `view!`).
#![allow(non_snake_case)]

mod api;
mod modes;
mod nav;
mod panes;
mod shell;

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::api::load_catalog_flags;
use crate::shell::Shell;

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
