//! Viewer tab: markdown (host HTML) and placeholders for other categories.

use leptos::html::Div;
use leptos::prelude::*;

use crate::api::{EntryContent, fetch_entry_content};

/// True when catalog category is Zahir Markdown (same gate as TUI viewer).
pub(crate) fn is_markdown_category(category: &str) -> bool {
    category == "Markdown"
}

#[component]
pub(crate) fn EntryViewer(path: String, category: String) -> impl IntoView {
    let show_category = !category.is_empty();
    let is_md = is_markdown_category(&category);

    view! {
        <div class="entry-viewer">
            <p class="entry-viewer__path">{path.clone()}</p>
            <Show when=move || show_category>
                <p class="entry-viewer__meta">{category.clone()}</p>
            </Show>
            {if is_md {
                view! { <MarkdownBody path=path/> }.into_any()
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
fn MarkdownBody(path: String) -> impl IntoView {
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
                Some(Ok(EntryContent { format, content, .. })) => {
                    if format == "html" {
                        view! { <MarkdownHtml html=content/> }.into_any()
                    } else {
                        view! { <pre class="detail-pre">{content}</pre> }.into_any()
                    }
                }
            }}
        </Suspense>
    }
}

#[component]
fn MarkdownHtml(html: String) -> impl IntoView {
    let node_ref = NodeRef::<Div>::new();
    Effect::new(move |_| {
        if let Some(el) = node_ref.get() {
            el.set_inner_html(&html);
        }
    });
    view! { <div class="md-viewer" node_ref=node_ref></div> }
}
