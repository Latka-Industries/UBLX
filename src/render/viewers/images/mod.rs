//! Image and PDF raster preview: policy in [`raster_policy`]; TUI `ratatui-image` wiring in [`core`].

#[cfg(feature = "tui")]
mod core;
mod raster_policy;

#[cfg(feature = "tui")]
pub use core::*;
pub use raster_policy::{
    VIEWPORT_RASTER_IMAGE, VIEWPORT_RASTER_PDF, ViewportCellRasterBudget, downscale_with_max,
    max_edge_for_pdf_viewport_cells, max_edge_for_viewport_cells,
    tiered_max_dimension_for_file_size,
};
