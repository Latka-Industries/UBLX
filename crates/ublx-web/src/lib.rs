//! UBLX embedded web UI (Leptos CSR + leptos-shadcn-ui).
//!
//! WASM CSR app — `./build.sh` / `mise run web`. Host `ublx` embeds `dist/` under
//! feature `ui` (see `src/cli/serve/web_embed.rs`); this crate is not published.
//!
//! Native workspace builds see an empty crate: all code and deps are wasm32-only.

#![cfg(target_arch = "wasm32")]
#![allow(non_snake_case)]

mod api;
mod catalog_data;
mod catalog_refresh;
mod command_mode;
mod entries_window;
mod focus;
mod help;
mod keys;
mod kv_tables;
mod marquee;
mod modes;
mod multiselect;
mod nav;
mod panes;
mod search;
mod shell;
mod snapshot_poll;
mod sort;
mod space_menu;
mod templates;
mod theme;
mod toast;
mod util;
mod viewer;
mod viewer_find;

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::api::{
    SettingsScope, fetch_settings, load_cached_catalog_flags, load_catalog_flags,
    persist_catalog_flags,
};
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
    // Soft-boot: seed from sessionStorage so a hard refresh skips "Connecting…".
    // Do **not** wrap Shell in Suspense — nested LocalResources would re-trip the splash.
    let cached = load_cached_catalog_flags();
    let flags = RwSignal::new(cached.clone().unwrap_or_default());
    let (booted, set_booted) = signal(cached.is_some());

    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(v) = fetch_settings(SettingsScope::Local).await {
                apply_theme_css_body(&v.css);
            }
        });
    });

    Effect::new(move |_| {
        spawn_local(async move {
            let fresh = load_catalog_flags().await;
            flags.set(persist_catalog_flags(fresh));
            // Only false → true. Setting `true` again would re-run the boot branch and
            // dispose Shell while its effects/resources are still live.
            if !booted.get_untracked() {
                set_booted.set(true);
            }
        });
    });

    view! {
        {move || {
            if !booted.get() {
                view! {
                    <div class="shell-boot">
                        <p class="shell-loading">"Connecting to UBLX…"</p>
                    </div>
                }
                .into_any()
            } else {
                // Only subscribe to `booted` here — background flag refresh updates the
                // signal in place so Shell does not remount.
                view! {
                    <div class="tui-shell">
                        <Shell flags=flags/>
                    </div>
                }
                .into_any()
            }
        }}
    }
}
