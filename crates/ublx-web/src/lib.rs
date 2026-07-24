//! UBLX embedded web UI (Leptos CSR + leptos-shadcn-ui).
//!
//! - **wasm32:** CSR app — `./build.sh` / `mise run web`.
//! - **host (`embed` feature):** [`embedded_assets`] for panza `StaticMount::Embedded`.

#![cfg_attr(target_arch = "wasm32", allow(non_snake_case))]

#[cfg(all(feature = "embed", not(target_arch = "wasm32")))]
mod embed;

#[cfg(all(feature = "embed", not(target_arch = "wasm32")))]
pub use embed::embedded_assets;

#[cfg(target_arch = "wasm32")]
mod api;
#[cfg(target_arch = "wasm32")]
mod catalog_refresh;
#[cfg(target_arch = "wasm32")]
mod command_mode;
#[cfg(target_arch = "wasm32")]
mod focus;
#[cfg(target_arch = "wasm32")]
mod help;
#[cfg(target_arch = "wasm32")]
mod keys;
#[cfg(target_arch = "wasm32")]
mod kv_tables;
#[cfg(target_arch = "wasm32")]
mod modes;
#[cfg(target_arch = "wasm32")]
mod multiselect;
#[cfg(target_arch = "wasm32")]
mod nav;
#[cfg(target_arch = "wasm32")]
mod panes;
#[cfg(target_arch = "wasm32")]
mod search;
#[cfg(target_arch = "wasm32")]
mod shell;
#[cfg(target_arch = "wasm32")]
mod sort;
#[cfg(target_arch = "wasm32")]
mod space_menu;
#[cfg(target_arch = "wasm32")]
mod theme;
#[cfg(target_arch = "wasm32")]
mod toast;
#[cfg(target_arch = "wasm32")]
mod util;
#[cfg(target_arch = "wasm32")]
mod viewer;
#[cfg(target_arch = "wasm32")]
mod viewer_find;

#[cfg(target_arch = "wasm32")]
use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(target_arch = "wasm32")]
use crate::api::{SettingsScope, fetch_settings, load_catalog_flags};
#[cfg(target_arch = "wasm32")]
use crate::shell::Shell;
#[cfg(target_arch = "wasm32")]
use crate::theme::apply_theme_css_body;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        view! { <App/> }
    });
}

#[cfg(target_arch = "wasm32")]
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
