//! SVG raster preview via **resvg** (same backend as Yazi).
//! Shell out → PNG on stdout (`-c`) → [`image::load_from_memory`].

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

/// Rasterize an SVG to a [`DynamicImage`], capped to `max_dim` on each axis.
///
/// Matches Yazi: `resvg -w N -h N --image-rendering optimizeSpeed <in> -c`.
///
/// # Errors
///
/// Returns a short message when `resvg` is missing, fails, or output is not a valid image.
pub fn rasterize(path: &Path, max_dim: u32) -> Result<DynamicImage, String> {
    let path_str = path.to_str().ok_or("invalid path")?;
    let dim = max_dim.to_string();

    let out = Command::new("resvg")
        .args([
            "-w",
            &dim,
            "-h",
            &dim,
            "--image-rendering",
            "optimizeSpeed",
            path_str,
            "-c",
        ])
        .output()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                "resvg not found. Install resvg for SVG preview (https://github.com/linebender/resvg)."
                    .to_string()
            } else {
                format!("resvg ({e})")
            }
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "resvg: failed".to_string()
        } else {
            format!("resvg: {stderr}")
        });
    }
    image::load_from_memory(&out.stdout).map_err(|e| e.to_string())
}
