//! Raw / PNG / PDF / video / cover `/content` responses.

use std::path::Path;

use axum::response::{IntoResponse, Response};

use crate::integrations::ZahirFT;
use crate::render::viewers::{images, pdf_preview, svg_preview, video_preview};
use crate::utils::try_extract_cover;

use super::super::error::ApiError;

/// Cap for passing through original bytes (`format=raw`). Larger / non-web formats → PNG preview.
const MAX_RAW_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

pub(super) fn ensure_image_previewable(abs: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(abs).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    if svg_preview::is_svg_path(abs) {
        if meta.len() > MAX_RAW_IMAGE_BYTES {
            return Err(format!("image larger than {MAX_RAW_IMAGE_BYTES} bytes"));
        }
        return Ok(());
    }
    if needs_png_preview(abs, meta.len()) {
        decode_png_preview(abs, meta.len()).map(|_| ())
    } else {
        Ok(())
    }
}

pub(super) fn ensure_tool_previewable(
    abs: &Path,
    zahir_type: Option<ZahirFT>,
    page: u32,
) -> Result<(), String> {
    let meta = std::fs::metadata(abs).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    match zahir_type {
        Some(ZahirFT::Pdf) => decode_pdf_preview(abs, page, meta.len()).map(|_| ()),
        Some(ZahirFT::Video) => decode_video_preview(abs, meta.len()).map(|_| ()),
        _ => Err("not a tool-backed preview category".into()),
    }
}

fn allows_raw_media(zahir_type: Option<ZahirFT>, abs: &Path) -> bool {
    matches!(
        zahir_type,
        Some(ZahirFT::Image | ZahirFT::Pdf | ZahirFT::Video)
    ) || svg_preview::is_svg_path(abs)
}

pub(super) fn raw_media_response(
    abs: &Path,
    zahir_type: Option<ZahirFT>,
    page: Option<u32>,
) -> Result<Response, ApiError> {
    if !allows_raw_media(zahir_type, abs) {
        return Err(ApiError::bad_request(
            "format=raw is only for Image, PDF, Video (or .svg) entries",
        ));
    }
    let meta = std::fs::metadata(abs).map_err(ApiError::not_found)?;
    if !meta.is_file() {
        return Err(ApiError::bad_request("not a file"));
    }
    match zahir_type {
        Some(ZahirFT::Pdf) => {
            return png_bytes_response(decode_pdf_preview(
                abs,
                page.unwrap_or(1).max(1),
                meta.len(),
            ));
        }
        Some(ZahirFT::Video) => return png_bytes_response(decode_video_preview(abs, meta.len())),
        _ => {}
    }
    // SVG stays vector; browsers can't show TIFF and many BMPs/huge rasters need a PNG preview.
    if svg_preview::is_svg_path(abs) {
        if meta.len() > MAX_RAW_IMAGE_BYTES {
            return Err(ApiError::bad_request(format!(
                "image larger than {MAX_RAW_IMAGE_BYTES} bytes"
            )));
        }
        let bytes = std::fs::read(abs).map_err(ApiError::not_found)?;
        return Ok(([(axum::http::header::CONTENT_TYPE, "image/svg+xml")], bytes).into_response());
    }
    if needs_png_preview(abs, meta.len()) {
        return png_preview_response(abs, meta.len());
    }
    let bytes = std::fs::read(abs).map_err(ApiError::not_found)?;
    let mime = image_mime_from_path(abs);
    Ok(([(axum::http::header::CONTENT_TYPE, mime)], bytes).into_response())
}

/// TIFF (no browser `<img>`), BMP/ICO, and oversize files → decode + PNG (TUI-tiered downscale).
fn needs_png_preview(path: &Path, len: u64) -> bool {
    if len > MAX_RAW_IMAGE_BYTES {
        return true;
    }
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("tif" | "tiff" | "bmp" | "dib" | "ico" | "tga")
    )
}

fn png_preview_response(abs: &Path, file_size: u64) -> Result<Response, ApiError> {
    png_bytes_response(decode_png_preview(abs, file_size))
}

fn png_bytes_response(result: Result<Vec<u8>, String>) -> Result<Response, ApiError> {
    let bytes = result.map_err(ApiError::bad_request)?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], bytes).into_response())
}

fn encode_png_bytes(img: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("encode png: {e}"))?;
    Ok(buf.into_inner())
}

fn decode_png_preview(abs: &Path, file_size: u64) -> Result<Vec<u8>, String> {
    let max_dim = images::tiered_max_dimension_for_file_size(file_size);
    let img = image::open(abs).map_err(|e| format!("decode image: {e}"))?;
    encode_png_bytes(&images::downscale_with_max(img, max_dim))
}

fn decode_pdf_preview(abs: &Path, page: u32, file_size: u64) -> Result<Vec<u8>, String> {
    let max_dim = pdf_preview::PdfRasterMaxDimBoost::apply(
        images::tiered_max_dimension_for_file_size(file_size),
    );
    let img = pdf_preview::render_pdf_page(abs, page, max_dim)?;
    encode_png_bytes(&images::downscale_with_max(img, max_dim))
}

fn decode_video_preview(abs: &Path, file_size: u64) -> Result<Vec<u8>, String> {
    let max_dim = images::tiered_max_dimension_for_file_size(file_size);
    let img = video_preview::decode_preview_frame(abs)?;
    encode_png_bytes(&images::downscale_with_max(img, max_dim))
}

pub(super) fn embedded_cover_response(
    abs: &Path,
    zahir_type: Option<ZahirFT>,
) -> Result<Response, ApiError> {
    let Some(ft @ (ZahirFT::Audio | ZahirFT::Epub)) = zahir_type else {
        return Err(ApiError::bad_request(
            "format=cover is only for Audio or Epub entries",
        ));
    };
    let Some(bytes) = try_extract_cover(abs, ft) else {
        return Err(ApiError::not_found("no embedded cover"));
    };
    // Covers may be JPEG/PNG (pass through) or uncommon codecs → PNG preview.
    let mime = image_mime_from_bytes(&bytes);
    if matches!(
        mime,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    ) {
        return Ok(([(axum::http::header::CONTENT_TYPE, mime)], bytes).into_response());
    }
    let img = image::load_from_memory(&bytes)
        .map_err(|e| ApiError::bad_request(format!("decode cover: {e}")))?;
    let img = images::downscale_with_max(
        img,
        images::tiered_max_dimension_for_file_size(bytes.len() as u64),
    );
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| ApiError::bad_request(format!("encode cover png: {e}")))?;
    Ok((
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        buf.into_inner(),
    )
        .into_response())
}

fn image_mime_from_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        Some("svg") => "image/svg+xml",
        Some("avif") => "image/avif",
        Some("tif" | "tiff") => "image/tiff",
        _ => "application/octet-stream",
    }
}

fn image_mime_from_bytes(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        let head = std::str::from_utf8(&bytes[..bytes.len().min(256)]).unwrap_or("");
        let trimmed = head.trim_start();
        if trimmed.starts_with("<svg")
            || trimmed.starts_with("<SVG")
            || trimmed.starts_with("<?xml")
        {
            "image/svg+xml"
        } else {
            "application/octet-stream"
        }
    }
}
