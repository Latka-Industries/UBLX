//! Viewer tab: markdown / syntect / CSV / text / image HTML (host) and placeholders for other categories.

use std::rc::Rc;

use leptos::html::Div;
use leptos::prelude::*;
use leptos_shadcn_ui::{Tooltip, TooltipContent, TooltipProvider};
use leptos_style::Style;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlElement;

use crate::api::{EntryContent, fetch_entry_content};

/// Zahir catalog category for markdown (`FileType::Markdown.as_metadata_name()`).
const MARKDOWN_CATEGORY: &str = "Markdown";

/// Categories that use syntect in the TUI Viewer (`viewer_uses_syntect_highlight`).
const SYNTECT_CATEGORIES: &[&str] = &["JSON", "TOML", "YAML", "XML", "HTML", "INI", "Log", "Code"];

const CSV_CATEGORY: &str = "CSV";
const TEXT_CATEGORY: &str = "Text";
const IMAGE_CATEGORY: &str = "Image";
/// Audio / Epub show embedded cover art in the TUI Viewer when present.
const COVER_CATEGORIES: &[&str] = &["Audio", "Epub"];

/// True when catalog category is Zahir Markdown (same gate as TUI viewer).
pub(crate) fn is_markdown_category(category: &str) -> bool {
    category == MARKDOWN_CATEGORY
}

pub(crate) fn is_syntect_category(category: &str) -> bool {
    SYNTECT_CATEGORIES.contains(&category)
}

pub(crate) fn is_csv_category(category: &str) -> bool {
    category == CSV_CATEGORY
}

pub(crate) fn is_text_category(category: &str) -> bool {
    category == TEXT_CATEGORY
}

pub(crate) fn is_image_category(category: &str) -> bool {
    category == IMAGE_CATEGORY
}

pub(crate) fn is_cover_category(category: &str) -> bool {
    COVER_CATEGORIES.contains(&category)
}

fn uses_html_viewer(category: &str, path: &str) -> bool {
    is_markdown_category(category)
        || is_syntect_category(category)
        || is_csv_category(category)
        || is_text_category(category)
        || is_image_category(category)
        || is_cover_category(category)
        || path_looks_delimited(path)
        || path_looks_svg(path)
}

fn path_looks_delimited(path: &str) -> bool {
    path.rsplit_once('.')
        .map(|(_, ext)| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "csv" | "tsv" | "tab" | "psv"
            )
        })
        .unwrap_or(false)
}

fn path_looks_svg(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("svg"))
}

fn viewer_html_class(category: &str, path: &str) -> &'static str {
    if is_markdown_category(category) {
        "md-viewer"
    } else if is_csv_category(category) || path_looks_delimited(path) {
        "csv-viewer"
    } else if is_text_category(category) {
        "text-viewer"
    } else if is_image_category(category) || is_cover_category(category) || path_looks_svg(path) {
        "img-viewer-host"
    } else {
        "code-viewer"
    }
}

#[component]
pub(crate) fn EntryViewer(path: String, category: String) -> impl IntoView {
    let html_viewer = uses_html_viewer(&category, &path);
    let class = viewer_html_class(&category, &path);

    view! {
        <div class="entry-viewer">
            {if html_viewer {
                view! { <HostHtmlBody path=path class=class/> }.into_any()
            } else {
                view! {
                    <p class="pane-empty entry-viewer__note">
                        "(viewer — preview for this category not available over serve yet)"
                    </p>
                }
                .into_any()
            }}
        </div>
    }
}

#[component]
fn HostHtmlBody(path: String, class: &'static str) -> impl IntoView {
    let path_for_fetch = path.clone();
    let content = LocalResource::new(move || {
        let p = path_for_fetch.clone();
        async move { fetch_entry_content(&p, Some("html")).await }
    });

    view! {
        <Suspense fallback=move || {
            view! { <p class="pane-empty">"Loading…"</p> }
        }>
            {move || match content.get() {
                None => view! { <p class="pane-empty">"…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="pane-empty">{e}</p> }.into_any(),
                Some(Ok(body)) => view! { <ContentBody body=body class=class/> }.into_any(),
            }}
        </Suspense>
    }
}

#[component]
fn ContentBody(body: EntryContent, class: &'static str) -> impl IntoView {
    if body.format == "html" {
        if class == "csv-viewer" {
            view! { <CsvHtmlFragment html=body.content/> }.into_any()
        } else {
            view! { <HtmlFragment class=class html=body.content/> }.into_any()
        }
    } else {
        view! { <pre class="detail-pre">{body.content}</pre> }.into_any()
    }
}

/// Trusted host HTML into a div (markdown / syntect / text / image).
#[component]
fn HtmlFragment(class: &'static str, html: String) -> impl IntoView {
    let node_ref = NodeRef::<Div>::new();
    Effect::new(move |_| {
        if let Some(el) = node_ref.get() {
            el.set_inner_html(&html);
            if class == "img-viewer-host" {
                wire_img_load_errors(&el);
            }
        }
    });
    view! { <div class=class node_ref=node_ref></div> }
}

/// If `<img>` fails, keep the broken icon and show the serve error underneath.
fn wire_img_load_errors(root: &web_sys::HtmlDivElement) {
    let Ok(nodes) = root.query_selector_all("img.img-viewer__img") else {
        return;
    };
    for i in 0..nodes.length() {
        let Some(node) = nodes.item(i) else {
            continue;
        };
        let Ok(img) = node.dyn_into::<web_sys::HtmlImageElement>() else {
            continue;
        };
        let host = root.clone();
        let on_err = Closure::wrap(Box::new(move |ev: web_sys::Event| {
            let Some(target) = ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlImageElement>().ok())
            else {
                return;
            };
            let src = target.current_src();
            if src.is_empty() {
                show_img_error_under(&host, "(failed to load image)");
                return;
            }
            let host = host.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let msg = match gloo_net::http::Request::get(&src).send().await {
                    Ok(resp) if !resp.ok() => resp
                        .json::<serde_json::Value>()
                        .await
                        .ok()
                        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_owned))
                        .unwrap_or_else(|| format!("failed to load image ({})", resp.status())),
                    Ok(_) => "(failed to load image)".into(),
                    Err(e) => e.to_string(),
                };
                show_img_error_under(&host, &msg);
            });
        }) as Box<dyn FnMut(_)>);
        let _ = img.add_event_listener_with_callback("error", on_err.as_ref().unchecked_ref());
        on_err.forget();
    }
}

fn show_img_error_under(host: &web_sys::HtmlDivElement, msg: &str) {
    if host
        .query_selector(".img-viewer__empty")
        .ok()
        .flatten()
        .is_some()
    {
        return;
    }
    let escaped = html_escape_text(msg);
    let _ = host.insert_adjacent_html(
        "beforeend",
        &format!(r#"<p class="img-viewer__empty">{escaped}</p>"#),
    );
}

fn html_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// CSV host HTML + frozen H/V scroll + shadcn tooltip for truncated cells.
#[component]
fn CsvHtmlFragment(html: String) -> impl IntoView {
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

/// Frozen top H-bar + side V-bar; table follows via `translate(-x, -y)`.
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

    let apply_transform: Rc<dyn Fn()> = Rc::new({
        let inner = inner.clone();
        let hbar = hbar.clone();
        let vbar = vbar.clone();
        move || {
            let x = hbar.scroll_left();
            let y = vbar.scroll_top();
            let _ = inner
                .style()
                .set_property("transform", &format!("translate(-{x}px, -{y}px)"));
        }
    });

    let sync_spacers: Rc<dyn Fn()> = Rc::new({
        let hspacer = hspacer.clone();
        let vspacer = vspacer.clone();
        let inner = inner.clone();
        move || {
            let w = inner.scroll_width().max(0);
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
    let _ = body.add_event_listener_with_callback("wheel", on_wheel.as_ref().unchecked_ref());
    on_wheel.forget();

    if let Some(window) = web_sys::window() {
        let sync_resize = Rc::clone(&sync_spacers);
        let apply_resize = Rc::clone(&apply_transform);
        let on_resize = Closure::wrap(Box::new(move |_: web_sys::Event| {
            sync_resize();
            apply_resize();
        }) as Box<dyn FnMut(_)>);
        let _ =
            window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
        on_resize.forget();
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
