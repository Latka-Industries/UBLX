//! Conveyor-belt horizontal scroll for overflowing one-line panel labels (TUI marquee parity).

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::focus::{UiNav, ViewerFullscreen};

/// ~110ms per character — matches TUI `MARQUEE_STEP`.
const MARQUEE_MS_PER_CHAR: f64 = 110.0;
/// Gap between loop end and start (`white-space: pre` required so spaces are not collapsed).
const MARQUEE_PAD: &str = "    ";

fn marquee_duration_secs(label: &str) -> f64 {
    let cycle_chars = (label.chars().count() + MARQUEE_PAD.chars().count()).max(1) as f64;
    (cycle_chars * MARQUEE_MS_PER_CHAR / 1000.0).max(0.5)
}

/// Selected + pane-focused row label: ellipsis when idle, conveyor scroll when it overflows.
#[component]
pub(crate) fn PanelRowText(label: String, selected: Signal<bool>) -> impl IntoView {
    let host_ref = NodeRef::<leptos::html::Span>::new();
    let measure_ref = NodeRef::<leptos::html::Span>::new();
    let (scrolling, set_scrolling) = signal(false);
    let (duration_s, set_duration_s) = signal(8.0_f64);
    let nav = UiNav::expect();
    let fullscreen = ViewerFullscreen::expect();
    let duration_for_label = marquee_duration_secs(&label);
    let segment = format!("{label}{MARQUEE_PAD}");

    Effect::new(move |_| {
        let _ = nav.pane.get();
        let _ = fullscreen.active.get();
        if !selected.get() {
            set_scrolling.set(false);
            return;
        }

        let Some(window) = web_sys::window() else {
            return;
        };
        let host_ref = host_ref;
        let measure_ref = measure_ref;
        let set_scrolling = set_scrolling;
        let set_duration_s = set_duration_s;

        let cb = Closure::once_into_js(move || {
            let Some(host) = host_ref.get() else {
                set_scrolling.set(false);
                return;
            };
            let Ok(Some(_)) = host.closest(".panel--focused") else {
                set_scrolling.set(false);
                return;
            };
            if host
                .closest(".three-pane--viewer-fullscreen")
                .ok()
                .flatten()
                .is_some()
            {
                set_scrolling.set(false);
                return;
            }
            let Some(measure) = measure_ref.get() else {
                set_scrolling.set(false);
                return;
            };
            let overflow = measure.scroll_width() > host.client_width() + 1;
            if !overflow {
                set_scrolling.set(false);
                return;
            }
            set_duration_s.set(duration_for_label);
            set_scrolling.set(true);
        });
        let _ = window.request_animation_frame(cb.as_ref().unchecked_ref());
    });

    view! {
        <span
            node_ref=host_ref
            class=move || {
                if scrolling.get() {
                    "panel-row__text panel-row__text--marquee"
                } else {
                    "panel-row__text"
                }
            }
            title=label.clone()
        >
            // Full-width measure (hidden) so ellipsis does not hide overflow.
            <span
                node_ref=measure_ref
                class="panel-row__measure"
                aria-hidden="true"
            >
                {label.clone()}
            </span>
            <Show
                when=move || scrolling.get()
                fallback={
                    let idle = label.clone();
                    move || view! { <span class="panel-row__text-inner">{idle.clone()}</span> }
                }
            >
                <span
                    class="panel-row__marquee-track"
                    style=move || format!("animation-duration: {}s", duration_s.get())
                >
                    <span class="panel-row__marquee-seg">{segment.clone()}</span>
                    <span class="panel-row__marquee-seg">{segment.clone()}</span>
                </span>
            </Show>
        </span>
    }
}
