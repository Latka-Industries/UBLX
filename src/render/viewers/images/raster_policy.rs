//! Raster decode caps: viewport cell→pixel budgets, file-size tiers, and downscale helper.

use crate::utils::MIB;

/// Cell→pixel estimates for longest-edge raster budgets.
/// Sized for Kitty/iTerm graphics protocols (not half-block floor); still capped by file-size tiers.
#[derive(Clone, Copy, Debug)]
pub struct ViewportCellRasterBudget {
    pub px_per_cell_w: u32,
    pub px_per_cell_h: u32,
    pub min_longest_edge: u32,
}

impl ViewportCellRasterBudget {
    #[must_use]
    pub fn max_edge_for_cells(self, width_cells: u16, height_cells: u16) -> u32 {
        let w = u32::from(width_cells);
        let h = u32::from(height_cells);
        let by_w = w.saturating_mul(self.px_per_cell_w);
        let by_h = h.saturating_mul(self.px_per_cell_h);
        by_w.max(by_h).max(self.min_longest_edge)
    }
}

pub const VIEWPORT_RASTER_IMAGE: ViewportCellRasterBudget = ViewportCellRasterBudget {
    px_per_cell_w: 20,
    px_per_cell_h: 40,
    min_longest_edge: 800,
};

/// Slightly **taller** cell→px vertical term so PDF pages (often portrait) get a bit more longest-edge budget.
pub const VIEWPORT_RASTER_PDF: ViewportCellRasterBudget = ViewportCellRasterBudget {
    px_per_cell_w: 20,
    px_per_cell_h: 48,
    min_longest_edge: 960,
};

/// Upper bound on longest edge (px) from the preview **area in terminal cells** so we don’t decode
/// more pixels than can appear in the pane (half-blocks ≈ a few px per cell; this is conservative).
#[must_use]
pub fn max_edge_for_viewport_cells(width_cells: u16, height_cells: u16) -> u32 {
    VIEWPORT_RASTER_IMAGE.max_edge_for_cells(width_cells, height_cells)
}

/// Like [`max_edge_for_viewport_cells`], but uses [`VIEWPORT_RASTER_PDF`].
#[must_use]
pub fn max_edge_for_pdf_viewport_cells(width_cells: u16, height_cells: u16) -> u32 {
    VIEWPORT_RASTER_PDF.max_edge_for_cells(width_cells, height_cells)
}

/// Longest edge (px) after decode, tiered by **file size** (smaller caps for heavier files = less work in `thumbnail` + terminal encode).
#[must_use]
pub fn tiered_max_dimension_for_file_size(file_size_bytes: u64) -> u32 {
    match file_size_bytes {
        s if s >= 32 * MIB => 768,
        s if s >= 20 * MIB => 1024,
        s if s >= 8 * MIB => 1280,
        s if s >= 2 * MIB => 1600,
        s if s >= MIB => 1440,
        _ => 1600,
    }
}

#[inline]
#[must_use]
pub fn downscale_with_max(img: image::DynamicImage, max_dim: u32) -> image::DynamicImage {
    let w = img.width();
    let h = img.height();
    if w <= max_dim && h <= max_dim {
        img
    } else {
        img.thumbnail(max_dim, max_dim)
    }
}
