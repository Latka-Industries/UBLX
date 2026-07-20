//! SVG raster preview via **ImageMagick** [`magick`] (legacy [`convert`] fallback).
//! Mirrors [`super::video_preview`]: shell out → PNG bytes → [`image::load_from_memory`].

use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use image::DynamicImage;

/// True when `path` looks like an SVG by extension (case-insensitive).
#[must_use]
pub fn is_svg_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
}

/// Rasterize an SVG to a [`DynamicImage`], capped to `max_dim` on the long edge.
///
/// Prefers `magick` (ImageMagick 7); falls back to legacy `convert` if `magick` is missing.
///
/// # Errors
///
/// Returns a short message when neither binary is available, the tool fails, or output is not a valid image.
pub fn rasterize(path: &Path, max_dim: u32) -> Result<DynamicImage, String> {
    let path_str = path.to_str().ok_or("invalid path")?;
    let geom = format!("{max_dim}x{max_dim}");

    let magick_err = match try_rasterize("magick", path_str, &geom) {
        Ok(img) => return Ok(img),
        Err(e) if e.missing_binary => e,
        Err(e) => return Err(e.message),
    };

    match try_rasterize("convert", path_str, &geom) {
        Ok(img) => Ok(img),
        Err(e) if e.missing_binary && magick_err.missing_binary => Err(
            "ImageMagick not found. Install ImageMagick (magick) for SVG preview (https://imagemagick.org)."
                .to_string(),
        ),
        Err(e) if e.missing_binary => Err(magick_err.message),
        Err(e) => Err(e.message),
    }
}

struct ToolAttempt {
    missing_binary: bool,
    message: String,
}

/// DPI used when ImageMagick rasterizes the SVG (before `-resize`); higher = sharper curves.
const SVG_RASTER_DENSITY: &str = "192";

fn try_rasterize(bin: &str, path_str: &str, geom: &str) -> Result<DynamicImage, ToolAttempt> {
    // `-density` must precede the SVG input so vectors are sampled at that DPI.
    let out = Command::new(bin)
        .args([
            "-density",
            SVG_RASTER_DENSITY,
            path_str,
            "-background",
            "none",
            "-resize",
            geom,
            "PNG:-",
        ])
        .output()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                ToolAttempt {
                    missing_binary: true,
                    message: format!("{bin} not found"),
                }
            } else {
                ToolAttempt {
                    missing_binary: false,
                    message: format!("{bin} ({e})"),
                }
            }
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(ToolAttempt {
            missing_binary: false,
            message: if stderr.is_empty() {
                format!("{bin}: failed")
            } else {
                format!("{bin}: {stderr}")
            },
        });
    }
    image::load_from_memory(&out.stdout).map_err(|e| ToolAttempt {
        missing_binary: false,
        message: e.to_string(),
    })
}
