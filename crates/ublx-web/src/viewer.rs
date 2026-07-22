//! Viewer tab: markdown / syntect HTML (host) and placeholders for other categories.

use leptos::html::Div;
use leptos::prelude::*;

use crate::api::{EntryContent, fetch_entry_content};

/// Zahir catalog category for markdown (`FileType::Markdown.as_metadata_name()`).
const MARKDOWN_CATEGORY: &str = "Markdown";

/// Categories that use syntect in the TUI Viewer (`viewer_uses_syntect_highlight`).
const SYNTECT_CATEGORIES: &[&str] = &["JSON", "TOML", "YAML", "XML", "HTML", "INI", "Log", "Code"];

/// True when catalog category is Zahir Markdown (same gate as TUI viewer).
pub(crate) fn is_markdown_category(category: &str) -> bool {
    category == MARKDOWN_CATEGORY
}

pub(crate) fn is_syntect_category(category: &str) -> bool {
    SYNTECT_CATEGORIES.contains(&category)
}

fn uses_html_viewer(category: &str) -> bool {
    is_markdown_category(category) || is_syntect_category(category)
}

fn viewer_html_class(category: &str) -> &'static str {
    if is_markdown_category(category) {
        "md-viewer"
    } else {
        "code-viewer"
    }
}

#[component]
pub(crate) fn EntryViewer(path: String, category: String) -> impl IntoView {
    let html_viewer = uses_html_viewer(&category);
    let class = viewer_html_class(&category);

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
        view! { <HtmlFragment class=class html=body.content/> }.into_any()
    } else {
        view! { <pre class="detail-pre">{body.content}</pre> }.into_any()
    }
}

/// Trusted host HTML into a div (markdown / syntect / future viewers).
#[component]
fn HtmlFragment(class: &'static str, html: String) -> impl IntoView {
    let node_ref = NodeRef::<Div>::new();
    Effect::new(move |_| {
        if let Some(el) = node_ref.get() {
            el.set_inner_html(&html);
        }
    });
    view! { <div class=class node_ref=node_ref></div> }
}
