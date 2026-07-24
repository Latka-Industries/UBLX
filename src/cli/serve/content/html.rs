//! HTML body generation for `/content` (markdown, syntect, CSV, image hosts).

use std::fmt::Write as _;
use std::path::Path;

use crate::cli::remote::encode_entry_path;
use crate::integrations::{ZahirFT, delimiter_from_path_for_viewer};
use crate::render::viewers::{csv_handler, html_escape_minimal, svg_preview, syntect_text};
use crate::utils::try_extract_cover;

use super::media::{ensure_image_previewable, ensure_tool_previewable};

pub(super) fn content_to_html(
    text: &str,
    path: &str,
    abs: &Path,
    zahir_type: Option<ZahirFT>,
    palette: &crate::themes::Palette,
    page: Option<u32>,
) -> String {
    match zahir_type {
        Some(ZahirFT::Markdown) => markdown_to_html(text),
        Some(ft) if syntect_text::uses_syntect_ft(ft) => {
            syntect_text::highlight_viewer_html(text, path, ft, palette)
        }
        Some(ZahirFT::Csv) => csv_handler::delimited_to_html(text, path),
        Some(ZahirFT::Text) => format!("<pre>{}</pre>", html_escape_minimal(text)),
        Some(ZahirFT::Image) => image_viewer_html(path, abs, None),
        Some(ZahirFT::Pdf) => image_viewer_html(path, abs, Some(("raw", page.unwrap_or(1)))),
        Some(ZahirFT::Video) => image_viewer_html(path, abs, Some(("raw", 0))),
        Some(ft @ (ZahirFT::Audio | ZahirFT::Epub)) => {
            if try_extract_cover(abs, ft).is_some() {
                image_preview_html(path, "cover", None)
            } else {
                r#"<p class="img-viewer__empty">(no embedded cover)</p>"#.into()
            }
        }
        _ if svg_preview::is_svg_path(Path::new(path)) => image_viewer_html(path, abs, None),
        _ if delimiter_from_path_for_viewer(path).is_some() => {
            csv_handler::delimited_to_html(text, path)
        }
        _ => format!("<pre>{}</pre>", html_escape_minimal(text)),
    }
}

/// `<img>` plus an in-pane note when preview already fails (TIFF / missing tools / etc.).
///
/// `raw_opts`: `Some(("raw", page))` for PDF (`page` 1-based); `Some(("raw", 0))` for video;
/// `None` for Image/SVG (plain `?format=raw`).
fn image_viewer_html(rel_path: &str, abs: &Path, raw_opts: Option<(&str, u32)>) -> String {
    let img = match raw_opts {
        Some(("raw", page)) if page >= 1 => image_preview_html(rel_path, "raw", Some(page)),
        Some(("raw", _)) | None => image_preview_html(rel_path, "raw", None),
        Some((fmt, _)) => image_preview_html(rel_path, fmt, None),
    };
    let check = match raw_opts {
        Some(("raw", page)) if page >= 1 => ensure_tool_previewable(abs, Some(ZahirFT::Pdf), page),
        Some(("raw", _)) => ensure_tool_previewable(abs, Some(ZahirFT::Video), 1),
        _ => ensure_image_previewable(abs),
    };
    match check {
        Ok(()) => img,
        Err(msg) => format!(
            r#"{img}<p class="img-viewer__empty">{}</p>"#,
            html_escape_minimal(&msg)
        ),
    }
}

fn image_preview_html(rel_path: &str, format_query: &str, page: Option<u32>) -> String {
    let mut src = format!(
        "/content/{}?format={format_query}",
        encode_entry_path(rel_path)
    );
    if let Some(p) = page {
        let _ = write!(src, "&page={p}");
    }
    let alt = html_escape_minimal(rel_path);
    format!(
        r#"<div class="img-viewer"><img class="img-viewer__img" src="{src}" alt="{alt}" loading="lazy" /></div>"#
    )
}

fn markdown_to_html(src: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let parser = Parser::new_ext(src, Options::all());
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}
