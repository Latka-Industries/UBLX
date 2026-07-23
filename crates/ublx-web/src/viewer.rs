//! Viewer tab: markdown / syntect / CSV / text / image / PDF / video HTML (host) and placeholders.

use std::rc::Rc;

use leptos::html::Div;
use leptos::prelude::*;
use leptos_shadcn_ui::{Tooltip, TooltipContent, TooltipProvider};
use leptos_style::Style;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlElement;

use crate::api::{
    CONTENT_WINDOW_BYTES, CONTENT_WINDOW_MIN_FILE_BYTES, EntryContent, encode_entry_path,
    fetch_entry_content, fetch_entry_content_page, fetch_entry_content_window,
};
use crate::focus::{PdfPageCtl, PdfPageNav, PreviewKeysBus, TextWindowCtl};
use crate::kv_tables::CollapsibleTree;
use crate::viewer_find::ViewerFind;

/// Zahir catalog category for markdown (`FileType::Markdown.as_metadata_name()`).
const MARKDOWN_CATEGORY: &str = "Markdown";

/// Categories that use syntect in the TUI Viewer (`viewer_uses_syntect_highlight`).
const SYNTECT_CATEGORIES: &[&str] = &["JSON", "TOML", "YAML", "XML", "HTML", "INI", "Log", "Code"];

const CSV_CATEGORY: &str = "CSV";
const TEXT_CATEGORY: &str = "Text";
const IMAGE_CATEGORY: &str = "Image";
const PDF_CATEGORY: &str = "PDF";
const VIDEO_CATEGORY: &str = "Video";
const DIRECTORY_CATEGORY: &str = "Directory";
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

pub(crate) fn is_pdf_category(category: &str) -> bool {
    category == PDF_CATEGORY
}

pub(crate) fn is_video_category(category: &str) -> bool {
    category == VIDEO_CATEGORY
}

pub(crate) fn is_cover_category(category: &str) -> bool {
    COVER_CATEGORIES
        .iter()
        .any(|c| category.eq_ignore_ascii_case(c))
}

pub(crate) fn is_directory_category(category: &str) -> bool {
    category == DIRECTORY_CATEGORY || category.eq_ignore_ascii_case("Zarr")
}

fn path_looks_epub(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("epub"))
}

fn uses_html_viewer(category: &str, path: &str) -> bool {
    is_markdown_category(category)
        || is_syntect_category(category)
        || is_csv_category(category)
        || is_text_category(category)
        || is_image_category(category)
        || is_pdf_category(category)
        || is_video_category(category)
        || is_cover_category(category)
        || path_looks_epub(path)
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
    } else if is_image_category(category)
        || is_pdf_category(category)
        || is_video_category(category)
        || is_cover_category(category)
        || path_looks_epub(path)
        || path_looks_svg(path)
    {
        "img-viewer-host"
    } else {
        "code-viewer"
    }
}

#[component]
pub(crate) fn EntryViewer(path: String, category: String, size: u64) -> impl IntoView {
    let html_viewer = uses_html_viewer(&category, &path);
    let class = viewer_html_class(&category, &path);
    let is_pdf = is_pdf_category(&category);
    let is_cover = is_cover_category(&category) || path_looks_epub(&path);
    let windowed = wants_byte_window(&category, size);

    view! {
        <div class="entry-viewer">
            {if is_pdf {
                view! { <PdfViewer path=path/> }.into_any()
            } else if is_cover {
                view! { <CoverViewer path=path/> }.into_any()
            } else if is_directory_category(&category) {
                view! { <DirectoryTreeViewer path=path/> }.into_any()
            } else if windowed {
                view! { <WindowedTextViewer path=path size=size/> }.into_any()
            } else if html_viewer {
                view! { <HostHtmlBody path=path class=class/> }.into_any()
            } else {
                // Same body as TUI: `file_content_for_viewer` → e.g. "PARQUET file" / tet info.
                view! { <HostTextBody path=path/> }.into_any()
            }}
        </div>
    }
}

/// Audio / Epub: embedded cover via `/content?format=cover` (same bytes as TUI).
#[component]
fn CoverViewer(path: String) -> impl IntoView {
    let (load_err, set_load_err) = signal::<Option<String>>(None);
    let src = format!("/content/{}?format=cover", encode_entry_path(&path));
    let alt = path.clone();

    view! {
        <div class="img-viewer-host cover-viewer">
            <div class="img-viewer">
                <img
                    class="img-viewer__img"
                    src=src.clone()
                    alt=alt
                    loading="lazy"
                    on:error=move |_| {
                        let src = src.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let msg = match gloo_net::http::Request::get(&src).send().await {
                                Ok(resp) if !resp.ok() => resp
                                    .json::<serde_json::Value>()
                                    .await
                                    .ok()
                                    .and_then(|v| {
                                        v.get("error").and_then(|e| e.as_str()).map(str::to_owned)
                                    })
                                    .unwrap_or_else(|| {
                                        format!("failed to load cover ({})", resp.status())
                                    }),
                                Ok(_) => "(no embedded cover)".into(),
                                Err(e) => e.to_string(),
                            };
                            set_load_err.set(Some(msg));
                        });
                    }
                    on:load=move |_| set_load_err.set(None)
                />
            </div>
            <Show when=move || load_err.get().is_some()>
                <p class="img-viewer__empty">{move || load_err.get().unwrap_or_default()}</p>
            </Show>
        </div>
    }
}

/// Directory / Zarr store Viewer: collapsible fs tree from `/content` (`format=tree`).
#[component]
fn DirectoryTreeViewer(path: String) -> impl IntoView {
    let path_for_fetch = path.clone();
    let content = LocalResource::new(move || {
        let p = path_for_fetch.clone();
        async move { fetch_entry_content(&p, None).await }
    });

    view! {
        <Suspense fallback=move || {
            view! { <p class="pane-empty">"Loading…"</p> }
        }>
            {move || match content.get() {
                None => view! { <p class="pane-empty">"…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="pane-empty">{e}</p> }.into_any(),
                Some(Ok(body)) => {
                    if let Some(roots) = body.tree.filter(|t| !t.is_empty()) {
                        view! { <CollapsibleTree roots=roots/> }.into_any()
                    } else if body.content.is_empty() {
                        view! { <p class="pane-empty">(empty)</p> }.into_any()
                    } else {
                        view! {
                            <div class="text-viewer">
                                <pre class="detail-pre">{body.content}</pre>
                            </div>
                        }
                        .into_any()
                    }
                }
            }}
        </Suspense>
    }
}

fn wants_byte_window(category: &str, size: u64) -> bool {
    size >= CONTENT_WINDOW_MIN_FILE_BYTES
        && (is_text_category(category) || is_syntect_category(category))
}

/// Explore #12: plain-text byte windows for large Text/Code (Shift+J/K/B/E).
#[component]
fn WindowedTextViewer(path: String, size: u64) -> impl IntoView {
    let (offset, set_offset) = signal(0_u64);
    let (byte_len, set_byte_len) = signal(0_u64);
    let (total, set_total) = signal(size);
    let (limit, set_limit) = signal(CONTENT_WINDOW_BYTES);
    let (body, set_body) = signal(String::new());
    let (err, set_err) = signal::<Option<String>>(None);
    let preview = PreviewKeysBus::expect();

    Effect::new({
        let path = path.clone();
        move |_| {
            let p = path.clone();
            let off = offset.get();
            let lim = limit.get();
            set_err.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                match fetch_entry_content_window(&p, off, lim).await {
                    Ok(c) => {
                        set_body.set(c.content);
                        if let Some(o) = c.offset {
                            set_offset.set(o);
                        }
                        if let Some(n) = c.byte_len {
                            set_byte_len.set(n);
                        }
                        if let Some(t) = c.total_bytes {
                            set_total.set(t);
                        }
                        if let Some(l) = c.limit {
                            set_limit.set(l);
                        }
                    }
                    Err(e) => set_err.set(Some(e)),
                }
            });
        }
    });

    Effect::new(move |_| {
        let ctl = TextWindowCtl {
            apply: Callback::new(move |nav| {
                let lim = limit.get_untracked().max(1);
                let tot = total.get_untracked();
                let cur = offset.get_untracked();
                let len = byte_len.get_untracked();
                let next = match nav {
                    PdfPageNav::Next => {
                        let n = cur.saturating_add(len.max(1));
                        if n >= tot { cur } else { n }
                    }
                    PdfPageNav::Prev => cur.saturating_sub(lim),
                    PdfPageNav::Top => 0,
                    PdfPageNav::Bottom => tot.saturating_sub(lim),
                };
                if next != cur {
                    set_offset.set(next);
                }
            }),
            offset: offset.into(),
            byte_len: byte_len.into(),
            total: total.into(),
        };
        preview.text_win.set(Some(ctl));
        on_cleanup(move || {
            preview.text_win.set(None);
        });
    });

    view! {
        <div class="text-viewer windowed-text-viewer">
            <Show when=move || err.get().is_some()>
                <p class="pane-empty">{move || err.get().unwrap_or_default()}</p>
            </Show>
            <pre class="detail-pre">{move || body.get()}</pre>
            <p class="windowed-text-viewer__hint">
                "Windowed plain text — Shift+J/K page · Shift+B/E ends"
            </p>
        </div>
    }
}

/// TUI-parity PDF Viewer: page raster via `/content?format=raw&page=N`; keys via shell Shift+J/K/B/E.
#[component]
fn PdfViewer(path: String) -> impl IntoView {
    let (page, set_page) = signal(1_u32);
    let (page_count, set_page_count) = signal::<Option<u32>>(None);
    let (load_err, set_load_err) = signal::<Option<String>>(None);
    let preview = PreviewKeysBus::expect();

    // Reset + probe page count when the catalog path changes.
    Effect::new({
        let path = path.clone();
        move |_| {
            let p = path.clone();
            set_page.set(1);
            set_page_count.set(None);
            set_load_err.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                match fetch_entry_content_page(&p, Some("html"), Some(1)).await {
                    Ok(body) => {
                        if let Some(n) = body.page_count {
                            set_page_count.set(Some(n.max(1)));
                        }
                        if let Some(pg) = body.page {
                            set_page.set(pg.max(1));
                        }
                    }
                    Err(e) => set_load_err.set(Some(e)),
                }
            });
        }
    });

    // Register PDF page handler for shell Shift+J/K/B/E + footer bumper goto.
    Effect::new(move |_| {
        let ctl = PdfPageCtl {
            apply: Callback::new(move |nav| {
                apply_pdf_page_action(nav, page, set_page, page_count);
            }),
            goto: Callback::new(move |n: u32| {
                goto_pdf_page(n, set_page, page_count);
            }),
            page: page.into(),
            page_count: page_count.into(),
        };
        preview.pdf.set(Some(ctl));
        on_cleanup(move || {
            preview.pdf.set(None);
        });
    });

    let img_src = {
        let path = path.clone();
        move || {
            format!(
                "/content/{}?format=raw&page={}",
                encode_entry_path(&path),
                page.get().max(1)
            )
        }
    };
    let alt = path.clone();

    view! {
        <div class="img-viewer-host pdf-viewer">
            <div class="img-viewer">
                <img
                    class="img-viewer__img"
                    src=img_src
                    alt=alt
                    loading="lazy"
                    on:error=move |_| {
                        let src = format!(
                            "/content/{}?format=raw&page={}",
                            encode_entry_path(&path),
                            page.get_untracked().max(1)
                        );
                        wasm_bindgen_futures::spawn_local(async move {
                            let msg = match gloo_net::http::Request::get(&src).send().await {
                                Ok(resp) if !resp.ok() => resp
                                    .json::<serde_json::Value>()
                                    .await
                                    .ok()
                                    .and_then(|v| {
                                        v.get("error").and_then(|e| e.as_str()).map(str::to_owned)
                                    })
                                    .unwrap_or_else(|| {
                                        format!("failed to load page ({})", resp.status())
                                    }),
                                Ok(_) => "(failed to load page)".into(),
                                Err(e) => e.to_string(),
                            };
                            set_load_err.set(Some(msg));
                        });
                    }
                    on:load=move |_| set_load_err.set(None)
                />
            </div>
            <Show when=move || load_err.get().is_some()>
                <p class="img-viewer__empty">{move || load_err.get().unwrap_or_default()}</p>
            </Show>
        </div>
    }
}

fn apply_pdf_page_action(
    action: PdfPageNav,
    page: ReadSignal<u32>,
    set_page: WriteSignal<u32>,
    page_count: ReadSignal<Option<u32>>,
) {
    let cur = page.get_untracked().max(1);
    let max = page_count.get_untracked();
    let next = match action {
        PdfPageNav::Next => {
            let n = cur.saturating_add(1);
            max.map_or(n, |m| n.min(m.max(1)))
        }
        PdfPageNav::Prev => cur.saturating_sub(1).max(1),
        PdfPageNav::Top => 1,
        PdfPageNav::Bottom => max.unwrap_or(cur).max(1),
    };
    if next != cur {
        set_page.set(next);
    }
}

fn goto_pdf_page(n: u32, set_page: WriteSignal<u32>, page_count: ReadSignal<Option<u32>>) {
    let max = page_count.get_untracked().unwrap_or(u32::MAX).max(1);
    set_page.set(n.clamp(1, max));
}

/// ~5 line steps like TUI [`PREVIEW_SCROLL_STEP_LINES`].
const PREVIEW_SCROLL_STEP_PX: i32 = 5 * 18;

/// Scroll the right-pane Viewer (markdown / code / CSV / image / templates / …).
pub(crate) fn scroll_right_preview(nav: PdfPageNav) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let candidates = [
        ".right-pane .csv-viewer__vbar",
        ".right-pane .csv-viewer__viewport",
        ".right-pane .img-viewer-host:not(.pdf-viewer)",
        ".right-pane .panel-pad",
    ];
    let mut el = None;
    for sel in candidates {
        if let Ok(Some(n)) = doc.query_selector(sel) {
            el = Some(n);
            break;
        }
    }
    let Some(el) = el else {
        return;
    };
    let Ok(html) = el.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    match nav {
        PdfPageNav::Next => {
            html.set_scroll_top(html.scroll_top().saturating_add(PREVIEW_SCROLL_STEP_PX));
        }
        PdfPageNav::Prev => {
            html.set_scroll_top(html.scroll_top().saturating_sub(PREVIEW_SCROLL_STEP_PX));
        }
        PdfPageNav::Top => html.set_scroll_top(0),
        PdfPageNav::Bottom => html.set_scroll_top(html.scroll_height()),
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

/// Plain text from serve (`file_content_for_viewer`) — binary stub labels, tet info, etc.
#[component]
fn HostTextBody(path: String) -> impl IntoView {
    let path_for_fetch = path.clone();
    let content = LocalResource::new(move || {
        let p = path_for_fetch.clone();
        async move { fetch_entry_content(&p, Some("text")).await }
    });

    view! {
        <Suspense fallback=move || {
            view! { <p class="pane-empty">"Loading…"</p> }
        }>
            {move || match content.get() {
                None => view! { <p class="pane-empty">"…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="pane-empty">{e}</p> }.into_any(),
                Some(Ok(body)) => {
                    if body.content.is_empty() {
                        view! { <p class="pane-empty">(empty)</p> }.into_any()
                    } else {
                        view! {
                            <div class="text-viewer">
                                <pre class="detail-pre">{body.content}</pre>
                            </div>
                        }
                        .into_any()
                    }
                }
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
            if let Some(find) = use_context::<ViewerFind>() {
                find.bump_content();
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
