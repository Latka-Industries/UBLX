//! Performance regression guards for the viewer read path and cache eviction ([#26], [#28]).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ublx::engine::cache::{self, ViewerContentIdentity};
use ublx::integrations::ZahirFT;
use ublx::layout::setup::{UblxState, ViewerDiskContentCache};
use ublx::render::viewer_cache;
use ublx::utils::{ViewerReadPolicy, file_content_for_viewer};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_file(label: &str, contents: impl AsRef<[u8]>) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("ublx-perf-{label}-{n}"));
    std::fs::write(&path, contents.as_ref()).unwrap();
    path
}

fn remove_temp(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[test]
fn large_text_preview_uses_head_and_tail() {
    let head = "HEAD-START\n";
    let tail = "\nTAIL-END";
    let pad_len = ViewerReadPolicy::HALF_MIB_BYTES as usize + 64;
    let mut body = vec![b'x'; pad_len];
    let head_bytes = head.as_bytes();
    let tail_bytes = tail.as_bytes();
    body[..head_bytes.len()].copy_from_slice(head_bytes);
    body[pad_len - tail_bytes.len()..].copy_from_slice(tail_bytes);

    let path = temp_file("head-tail", body);
    let preview = file_content_for_viewer(&path, Some(ZahirFT::Code)).expect("preview");
    remove_temp(&path);

    assert!(
        preview.contains("HEAD-START"),
        "expected head chunk in preview"
    );
    assert!(
        preview.contains("TAIL-END"),
        "expected tail chunk in preview"
    );
    assert!(
        preview.contains("bytes omitted"),
        "expected explicit truncation marker, got {} bytes",
        preview.len()
    );
}

#[test]
fn large_log_preview_still_uses_head_and_tail() {
    let pad_len = ViewerReadPolicy::HALF_MIB_BYTES as usize + 1;
    let path = temp_file("log-head-tail", vec![b'l'; pad_len]);
    let preview = file_content_for_viewer(&path, Some(ZahirFT::Log)).expect("preview");
    remove_temp(&path);

    assert!(preview.contains("bytes omitted"));
}

#[test]
fn large_text_preview_respects_byte_budget() {
    let total = ViewerReadPolicy::HALF_MIB_BYTES + (128 * 1024);
    let path = temp_file("budget", vec![b'a'; total as usize]);
    let preview = file_content_for_viewer(&path, None).expect("preview");
    remove_temp(&path);

    let marker_overhead = 128;
    let max = ViewerReadPolicy::HALF_MIB_BYTES_USIZE + marker_overhead;
    assert!(
        preview.len() <= max,
        "preview {} bytes exceeds budget ~{}",
        preview.len(),
        max
    );
}

#[test]
fn small_text_preview_loads_whole_file() {
    let path = temp_file("small", b"hello\nworld\n");
    let preview = file_content_for_viewer(&path, Some(ZahirFT::Code)).expect("preview");
    remove_temp(&path);

    assert_eq!(preview, "hello\nworld\n");
    assert!(!preview.contains("bytes omitted"));
}

#[test]
fn evict_viewer_caches_clears_retained_preview_state() {
    let mut state = UblxState::new();
    state.viewer_disk_cache = Some(ViewerDiskContentCache {
        rel_path: "a.txt".into(),
        category: "Code".into(),
        file_len: 1,
        modified: None,
        viewer_str: Some("cached".into()),
        embedded_cover_raster: Some(vec![1, 2, 3]),
        viewer_can_open: true,
    });
    state.viewer_preview_source = Some((
        "a.txt".into(),
        ViewerContentIdentity::LenOnly { len: 6 },
    ));
    state.csv_table_text_lru.insert(
        cache::ViewerTableCacheKey {
            path: "wide.csv".into(),
            content_width: 80,
            theme_name: "Shadow Index".into(),
            identity: ViewerContentIdentity::LenOnly { len: 1 },
        },
        cache::ViewerTextCacheEntry {
            path: "wide.csv".into(),
            content_width: 80,
            theme_name: "Shadow Index".into(),
            content_identity: ViewerContentIdentity::LenOnly { len: 1 },
            line_count: 1,
            text: ratatui::text::Text::from("row"),
            syntect: None,
        },
    );
    state.viewer_image.key = Some("img.png".into());

    viewer_cache::evict_viewer_caches_on_selection_change(&mut state);

    assert!(state.viewer_disk_cache.is_none());
    assert!(state.viewer_text_cache.is_none());
    assert!(state.viewer_preview_source.is_none());
    assert!(state.csv_table_text_lru.entries.is_empty());
    assert!(state.viewer_image.key.is_none());
    assert!(state.viewer_image.image_lru.is_empty());
}

/// Repeated preview reads for the same large file should stay within the byte budget ([#28] guard).
#[test]
fn repeated_large_preview_reads_do_not_accumulate_in_memory() {
    let total = ViewerReadPolicy::HALF_MIB_BYTES + (256 * 1024);
    let path = temp_file("repeat", vec![b'z'; total as usize]);
    let marker_overhead = 128;
    let max = ViewerReadPolicy::HALF_MIB_BYTES_USIZE + marker_overhead;

    for _ in 0..8 {
        let preview = file_content_for_viewer(&path, Some(ZahirFT::Markdown)).expect("preview");
        assert!(preview.len() <= max);
    }

    remove_temp(&path);
}
