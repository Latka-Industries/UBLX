//! Image / PDF viewer raster state.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use crate::render::viewers::pdf_preview::PDFPrefetch;

pub struct PDF {
    pub page: u32,
    pub page_count: Option<u32>,
    pub for_path: Option<PathBuf>,
    pub page_count_rx: Option<mpsc::Receiver<Result<u32, String>>>,
    pub prefetch_cancel: Arc<AtomicU64>,
    pub prefetch_earliest: Option<Instant>,
    pub prefetch_rx: Option<mpsc::Receiver<(String, Result<image::DynamicImage, String>)>>,
}

impl Default for PDF {
    fn default() -> Self {
        Self {
            page: 1,
            page_count: None,
            for_path: None,
            page_count_rx: None,
            prefetch_cancel: Arc::new(AtomicU64::new(0)),
            prefetch_earliest: None,
            prefetch_rx: None,
        }
    }
}

/// State for the image viewer in the right pane (`ratatui-image`, tiered downscale, optional background decode).
#[derive(Default)]
pub struct ViewerImageState {
    pub protocol: Option<ratatui_image::protocol::StatefulProtocol>,
    pub picker: Option<ratatui_image::picker::Picker>,
    /// Cache key: path display, or `path#pN` for PDF page `N`.
    pub key: Option<String>,
    /// When set, a background thread is decoding/downsizing; poll in [`crate::render::viewers::image::ensure_viewer_image`].
    pub decode_rx: Option<mpsc::Receiver<Result<image::DynamicImage, String>>>,
    pub err: Option<String>,
    /// Recent previews (not the current row). Size [`Self::LRU_CAP`] is tied to PDF prefetch (see [`ViewerImageState::LRU_CAP`]).
    pub image_lru: VecDeque<(String, ratatui_image::protocol::StatefulProtocol)>,
    /// PDF: one-based page; PDF: selected file this state applies to.
    pub pdf: PDF,
}

impl ViewerImageState {
    /// `PDFPrefetch::MAX_EXTRA_PAGES` prefetched PDFs (pages 2..) plus **four** slots to stash the previous page
    pub const LRU_EXTRA_SLOTS: usize = 4;
    pub const LRU_CAP: usize = PDFPrefetch::MAX_EXTRA_PAGES as usize + Self::LRU_EXTRA_SLOTS;

    /// Push a finished preview into the LRU ring; drops the oldest entry when full.
    pub fn push_lru(&mut self, path: String, proto: ratatui_image::protocol::StatefulProtocol) {
        while self.image_lru.len() >= Self::LRU_CAP {
            self.image_lru.pop_front();
        }
        self.image_lru.push_back((path, proto));
    }

    /// Remove and return a cached protocol for `path` if present.
    pub fn take_from_lru(
        &mut self,
        path: &str,
    ) -> Option<ratatui_image::protocol::StatefulProtocol> {
        let pos = self.image_lru.iter().position(|(k, _)| k == path)?;
        self.image_lru.remove(pos).map(|(_, proto)| proto)
    }

    /// Drop an LRU entry matching `key` so a prefetch can replace it.
    pub fn remove_lru_key(&mut self, key: &str) {
        if let Some(pos) = self.image_lru.iter().position(|(k, _)| k == key) {
            self.image_lru.remove(pos);
        }
    }

    /// Clear loaded image, error, and async decode channel; **retains** [`Self::picker`].
    /// Drops all rasters (no LRU retention) — use [`Self::stash_page_for_pdf_navigation`] for PDF page hops.
    pub fn clear(&mut self) {
        self.evict_rasters();
    }

    /// Drop current and cached rasters; cancel in-flight decode and PDF prefetch. Keeps [`Self::picker`].
    pub fn evict_rasters(&mut self) {
        self.pdf.prefetch_cancel.fetch_add(1, Ordering::SeqCst);
        self.pdf.prefetch_rx = None;
        self.pdf.prefetch_earliest = None;
        self.decode_rx = None;
        self.pdf.page_count_rx = None;
        self.err = None;
        self.key = None;
        self.protocol = None;
        self.image_lru.clear();
        self.pdf.for_path = None;
        self.pdf.page = 1;
        self.pdf.page_count = None;
    }

    /// Stash the current PDF page preview for back-navigation within the same file.
    pub fn stash_page_for_pdf_navigation(
        &mut self,
        page_key: String,
        proto: ratatui_image::protocol::StatefulProtocol,
    ) {
        self.push_lru(page_key, proto);
    }
}
