//! Viewer tab: markdown / syntect / CSV / text / image / PDF / video HTML (host) and placeholders.

mod bodies;
mod csv;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::api::CONTENT_WINDOW_MIN_FILE_BYTES;
use crate::focus::PdfPageNav;

use self::bodies::{
    CoverViewer, DirectoryTreeViewer, HostHtmlBody, HostTextBody, PdfViewer, WindowedTextViewer,
};

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

fn wants_byte_window(category: &str, size: u64) -> bool {
    size >= CONTENT_WINDOW_MIN_FILE_BYTES
        && (is_text_category(category) || is_syntect_category(category))
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
