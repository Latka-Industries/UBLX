//! Evict right-pane viewer caches when catalog selection changes ([#25]).

use crate::layout::setup::UblxState;
use crate::render::viewers::async_tools;

/// Clear text/async/disk viewer caches and drop image/PDF rasters when the middle-pane row changes.
pub fn evict_viewer_caches_on_selection_change(state: &mut UblxState) {
    state.viewer_text_cache = None;
    state.viewer_preview_source = None;
    state.csv_table_text_lru.clear();
    async_tools::reset_viewer_async(state);
    state.viewer_disk_cache = None;
    state.viewer_image.evict_rasters();
}
