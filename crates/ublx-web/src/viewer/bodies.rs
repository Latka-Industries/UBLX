//! Host-backed viewer bodies (cover, directory, windowed text, PDF, HTML/text).

use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::api::{
    CONTENT_WINDOW_BYTES, EntryContent, encode_entry_path, fetch_entry_content,
    fetch_entry_content_page, fetch_entry_content_window,
};
use crate::focus::{PdfPageCtl, PdfPageNav, PreviewKeysBus, TextWindowCtl};
use crate::kv_tables::CollapsibleTree;
use crate::viewer_find::ViewerFind;

use super::csv::CsvHtmlFragment;

/// Audio / Epub: embedded cover via `/content?format=cover` (same bytes as TUI).
#[component]
pub(super) fn CoverViewer(path: String) -> impl IntoView {
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
pub(super) fn DirectoryTreeViewer(path: String) -> impl IntoView {
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

/// Explore #12: plain-text byte windows for large Text/Code (Shift+J/K/B/E).
#[component]
pub(super) fn WindowedTextViewer(path: String, size: u64) -> impl IntoView {
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
pub(super) fn PdfViewer(path: String) -> impl IntoView {
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

#[component]
pub(super) fn HostHtmlBody(path: String, class: &'static str) -> impl IntoView {
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
pub(super) fn HostTextBody(path: String) -> impl IntoView {
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
pub(super) fn ContentBody(body: EntryContent, class: &'static str) -> impl IntoView {
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
pub(super) fn HtmlFragment(class: &'static str, html: String) -> impl IntoView {
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
