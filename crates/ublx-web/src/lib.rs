//! UBLX embedded web UI (Leptos CSR + leptos-shadcn-ui).
//!
//! WASM CSR app — `./build.sh` / `mise run web`. Host `ublx` embeds `dist/` under
//! feature `ui` (see `src/cli/serve/web_embed.rs`); this crate is not published.

#![cfg_attr(target_arch = "wasm32", allow(non_snake_case))]

#[cfg(target_arch = "wasm32")]
mod api;
#[cfg(target_arch = "wasm32")]
mod catalog_data;
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
mod snapshot_poll;
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
use crate::api::{
    SettingsScope, fetch_settings, load_cached_catalog_flags, load_catalog_flags,
    persist_catalog_flags,
};
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
