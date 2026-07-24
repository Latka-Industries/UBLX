//! Tiny shared browser helpers (sleep, overlay backdrop).

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Await `ms` via `window.setTimeout` (wasm — no `std::thread::sleep`).
pub(crate) async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let _ = web_sys::window().map(|w| {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        });
    });
    let _ = JsFuture::from(promise).await;
}

/// True when `mousedown` hit the overlay backdrop element (not a child panel).
pub(crate) fn is_backdrop_click(ev: &web_sys::MouseEvent, overlay_class: &str) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        .is_some_and(|t| t.class_list().contains(overlay_class))
}
