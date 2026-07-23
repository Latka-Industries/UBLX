//! CSV Viewer: frozen header/scrollbars and cell tooltips.

use std::rc::Rc;

use leptos::html::Div;
use leptos::prelude::*;
use leptos_shadcn_ui::{Tooltip, TooltipContent, TooltipProvider};
use leptos_style::Style;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlElement;

use crate::viewer_find::ViewerFind;

/// CSV host HTML + frozen H/V scroll + shadcn tooltip for truncated cells.
#[component]
pub(super) fn CsvHtmlFragment(html: String) -> impl IntoView {
    let node_ref = NodeRef::<Div>::new();
    let (tip_open, set_tip_open) = signal(false);
    let (tip_text, set_tip_text) = signal(String::new());
    let (tip_x, set_tip_x) = signal(0_i32);
    let (tip_y, set_tip_y) = signal(0_i32);

    Effect::new(move |_| {
        if let Some(el) = node_ref.get() {
            el.set_inner_html(&html);
            wire_csv_frozen_scroll(&el);
            wire_csv_tip_signals(&el, set_tip_open, set_tip_text, set_tip_x, set_tip_y);
            if let Some(find) = use_context::<ViewerFind>() {
                find.bump_content();
            }
        }
    });

    // Keep Tooltip "open" so TooltipContent mounts once (Children is single-use).
    let tooltip_mounted = Signal::derive(|| true);
    let tip_style = Signal::derive(move || {
        if tip_open.get() {
            Style::from(format!(
                "position: fixed; left: {}px; top: {}px; z-index: 80; max-height: 12rem; overflow-y: auto; display: block;",
                tip_x.get() + 14,
                tip_y.get() + 14
            ))
        } else {
            Style::from("display: none;")
        }
    });

    view! {
        <div class="csv-viewer-host">
            <div class="csv-viewer" node_ref=node_ref></div>
            <TooltipProvider>
                <Tooltip open=tooltip_mounted>
                    <TooltipContent
                        class="max-w-md whitespace-normal break-words"
                        style=tip_style
                    >
                        {move || tip_text.get()}
                    </TooltipContent>
                </Tooltip>
            </TooltipProvider>
        </div>
    }
}

fn qs_html(root: &HtmlElement, sel: &str) -> Option<HtmlElement> {
    root.query_selector(sel)
        .ok()
        .flatten()
        .and_then(|n| n.dyn_into::<HtmlElement>().ok())
}

fn listen_scroll(el: &HtmlElement, on_scroll: Rc<dyn Fn()>) {
    let cb = Closure::wrap(Box::new(move |_: web_sys::Event| {
        on_scroll();
    }) as Box<dyn FnMut(_)>);
    let _ = el.add_event_listener_with_callback("scroll", cb.as_ref().unchecked_ref());
    cb.forget();
}

/// Frozen top H-bar + side V-bar; pinned header (X only) + body (X/Y) via translate.
fn wire_csv_frozen_scroll(root: &web_sys::HtmlDivElement) {
    let Some(hbar) = qs_html(root, ".csv-viewer__hbar") else {
        return;
    };
    let Some(hspacer) = qs_html(root, ".csv-viewer__hspacer") else {
        return;
    };
    let Some(vbar) = qs_html(root, ".csv-viewer__vbar") else {
        return;
    };
    let Some(vspacer) = qs_html(root, ".csv-viewer__vspacer") else {
        return;
    };
    let Some(inner) = qs_html(root, ".csv-viewer__inner") else {
        return;
    };
    let Some(body) = qs_html(root, ".csv-viewer__body") else {
        return;
    };
    let head_inner = qs_html(root, ".csv-viewer__head-inner");
    let viewport = qs_html(root, ".csv-viewer__viewport").unwrap_or_else(|| body.clone());

    sync_csv_column_widths(root);

    let apply_transform: Rc<dyn Fn()> = Rc::new({
        let inner = inner.clone();
        let head_inner = head_inner.clone();
        let hbar = hbar.clone();
        let vbar = vbar.clone();
        move || {
            let x = hbar.scroll_left();
            let y = vbar.scroll_top();
            let _ = inner
                .style()
                .set_property("transform", &format!("translate(-{x}px, -{y}px)"));
            if let Some(ref head) = head_inner {
                let _ = head
                    .style()
                    .set_property("transform", &format!("translateX(-{x}px)"));
            }
        }
    });

    let sync_spacers: Rc<dyn Fn()> = Rc::new({
        let hspacer = hspacer.clone();
        let vspacer = vspacer.clone();
        let inner = inner.clone();
        let head_inner = head_inner.clone();
        move || {
            let body_w = inner.scroll_width().max(0);
            let head_w = head_inner
                .as_ref()
                .map(|h| h.scroll_width().max(0))
                .unwrap_or(0);
            let w = body_w.max(head_w);
            let h = inner.scroll_height().max(0);
            let _ = hspacer.style().set_property("width", &format!("{w}px"));
            let _ = vspacer.style().set_property("height", &format!("{h}px"));
        }
    });
    sync_spacers();
    apply_transform();

    listen_scroll(&hbar, Rc::clone(&apply_transform));
    listen_scroll(&vbar, Rc::clone(&apply_transform));

    let apply_wheel = Rc::clone(&apply_transform);
    let sync_wheel = Rc::clone(&sync_spacers);
    let on_wheel = Closure::wrap(Box::new(move |ev: web_sys::WheelEvent| {
        sync_wheel();
        let dx = ev.delta_x();
        let dy = ev.delta_y();
        let horizontal = dx.abs() > dy.abs() || ev.shift_key();
        ev.prevent_default();
        if horizontal {
            let delta = if ev.shift_key() && dx.abs() < f64::EPSILON {
                dy
            } else {
                dx
            };
            hbar.set_scroll_left(hbar.scroll_left() + delta as i32);
        } else {
            vbar.set_scroll_top(vbar.scroll_top() + dy as i32);
        }
        apply_wheel();
    }) as Box<dyn FnMut(_)>);
    let _ = viewport.add_event_listener_with_callback("wheel", on_wheel.as_ref().unchecked_ref());
    on_wheel.forget();

    if let Some(window) = web_sys::window() {
        let sync_resize = Rc::clone(&sync_spacers);
        let apply_resize = Rc::clone(&apply_transform);
        let root = root.clone();
        let on_resize = Closure::wrap(Box::new(move |_: web_sys::Event| {
            sync_csv_column_widths(&root);
            sync_resize();
            apply_resize();
        }) as Box<dyn FnMut(_)>);
        let _ =
            window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
        on_resize.forget();
    }
}

/// Align head/body column widths so the pinned header lines up with the data.
fn sync_csv_column_widths(root: &web_sys::HtmlDivElement) {
    let Some(head_table) = root
        .query_selector(".csv-viewer__head table")
        .ok()
        .flatten()
        .and_then(|n| n.dyn_into::<web_sys::HtmlTableElement>().ok())
    else {
        return;
    };
    let Some(body_table) = root
        .query_selector(".csv-viewer__body table")
        .ok()
        .flatten()
        .and_then(|n| n.dyn_into::<web_sys::HtmlTableElement>().ok())
    else {
        return;
    };
    let head_rows = head_table.rows();
    let body_rows = body_table.rows();
    let Some(head_row) = head_rows.item(0) else {
        return;
    };
    let Ok(head_row) = head_row.dyn_into::<web_sys::HtmlTableRowElement>() else {
        return;
    };
    let head_cells = head_row.cells();
    let col_count = head_cells.length() as usize;
    if col_count == 0 {
        return;
    }

    // Clear prior fixed widths so natural content can re-measure.
    for i in 0..col_count {
        if let Some(cell) = head_cells.item(i as u32)
            && let Ok(el) = cell.dyn_into::<HtmlElement>()
        {
            let _ = el.style().remove_property("width");
            let _ = el.style().remove_property("min-width");
        }
    }
    for r in 0..body_rows.length() {
        let Some(row) = body_rows.item(r) else {
            continue;
        };
        let Ok(row) = row.dyn_into::<web_sys::HtmlTableRowElement>() else {
            continue;
        };
        let cells = row.cells();
        for i in 0..col_count {
            if let Some(cell) = cells.item(i as u32)
                && let Ok(el) = cell.dyn_into::<HtmlElement>()
            {
                let _ = el.style().remove_property("width");
                let _ = el.style().remove_property("min-width");
            }
        }
    }

    let mut widths = vec![0_i32; col_count];
    for (i, width) in widths.iter_mut().enumerate() {
        if let Some(cell) = head_cells.item(i as u32)
            && let Ok(el) = cell.dyn_into::<HtmlElement>()
        {
            *width = (*width).max(el.offset_width());
        }
    }
    // Sample up to first ~40 body rows for width (enough for typical headers).
    let sample = body_rows.length().min(40);
    for r in 0..sample {
        let Some(row) = body_rows.item(r) else {
            continue;
        };
        let Ok(row) = row.dyn_into::<web_sys::HtmlTableRowElement>() else {
            continue;
        };
        let cells = row.cells();
        for (i, width) in widths.iter_mut().enumerate() {
            if let Some(cell) = cells.item(i as u32)
                && let Ok(el) = cell.dyn_into::<HtmlElement>()
            {
                *width = (*width).max(el.offset_width());
            }
        }
    }

    for (i, &width) in widths.iter().enumerate() {
        let w = format!("{}px", width.max(1));
        if let Some(cell) = head_cells.item(i as u32)
            && let Ok(el) = cell.dyn_into::<HtmlElement>()
        {
            let _ = el.style().set_property("width", &w);
            let _ = el.style().set_property("min-width", &w);
        }
        for r in 0..body_rows.length() {
            let Some(row) = body_rows.item(r) else {
                continue;
            };
            let Ok(row) = row.dyn_into::<web_sys::HtmlTableRowElement>() else {
                continue;
            };
            let cells = row.cells();
            if let Some(cell) = cells.item(i as u32)
                && let Ok(el) = cell.dyn_into::<HtmlElement>()
            {
                let _ = el.style().set_property("width", &w);
                let _ = el.style().set_property("min-width", &w);
            }
        }
    }
}

fn wire_csv_tip_signals(
    root: &web_sys::HtmlDivElement,
    set_open: WriteSignal<bool>,
    set_text: WriteSignal<String>,
    set_x: WriteSignal<i32>,
    set_y: WriteSignal<i32>,
) {
    let on_over = Closure::wrap(Box::new(move |ev: web_sys::MouseEvent| {
        let Some(target) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        let Some(cell) = tip_cell(&target) else {
            return;
        };
        let Some(text) = cell.get_attribute("data-tip") else {
            return;
        };
        if text.is_empty() {
            set_open.set(false);
            return;
        }
        set_text.set(text);
        set_x.set(ev.client_x());
        set_y.set(ev.client_y());
        set_open.set(true);
    }) as Box<dyn FnMut(_)>);

    let on_out = Closure::wrap(Box::new(move |ev: web_sys::MouseEvent| {
        let still = ev
            .related_target()
            .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
            .and_then(|e| tip_cell(&e))
            .is_some();
        if !still {
            set_open.set(false);
        }
    }) as Box<dyn FnMut(_)>);

    let on_move = Closure::wrap(Box::new(move |ev: web_sys::MouseEvent| {
        set_x.set(ev.client_x());
        set_y.set(ev.client_y());
    }) as Box<dyn FnMut(_)>);

    let _ = root.add_event_listener_with_callback("mouseover", on_over.as_ref().unchecked_ref());
    let _ = root.add_event_listener_with_callback("mouseout", on_out.as_ref().unchecked_ref());
    let _ = root.add_event_listener_with_callback("mousemove", on_move.as_ref().unchecked_ref());
    on_over.forget();
    on_out.forget();
    on_move.forget();
}

fn tip_cell(el: &web_sys::Element) -> Option<web_sys::Element> {
    el.closest("[data-tip]").ok().flatten()
}
