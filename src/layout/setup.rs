//! 3-panel TUI: categories (left), contents (middle), preview (right).
//!
//! [`crate::handlers::core::run_tui_session`] drives the loop; work per tick is split into four phases (see classification below).
//! Action application (key → state changes) lives in [`crate::handlers::state_transitions`].

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use ratatui::style::Style;
use ratatui::widgets::ListState;

use crate::engine::{
    cache,
    db_ops::{DeltaType, UblxDbCategory},
    viewer_async::ViewerAsyncState,
};
use crate::integrations::{ZahirFT, file_type_from_metadata_name};
use crate::render::viewers::pdf_preview::PDFPrefetch;
use crate::utils::{ClipboardCopyCommand, ToastSlot};

use super::style;

/// Re-export snapshot row type for layout/view/render (`path`, category, size).
pub use crate::engine::db_ops::SnapshotTuiRow as TuiRow;

/// Category string for directories in the snapshot (matches [`crate::engine::db_ops::UblxDbCategory`]).
pub const CATEGORY_DIRECTORY: &str = "Directory";

/// State for horizontal marquee when a list row label overflows (e.g. Duplicates/Lenses left pane).
#[derive(Debug, Default)]
pub struct ContentMarqueeState {
    pub offset: usize,
    pub last_advance: Option<Instant>,
    pub anchor: Option<(usize, String)>,
}

impl ContentMarqueeState {
    pub fn reset(&mut self) {
        self.offset = 0;
        self.last_advance = None;
        self.anchor = None;
    }
}

/// List panels: categories, contents, focus, preview scroll, and highlight style.
#[derive(Default)]
pub struct PanelState {
    pub category_state: ListState,
    pub content_state: ListState,
    pub focus: PanelFocus,
    pub preview_scroll: u16,
    pub prev_preview_key: Option<(usize, Option<usize>)>,
    pub highlight_style: Style,
    pub content_sort: ContentSort,
    /// Temporary anchor used to keep the same selected item identity after sort changes.
    pub sort_anchor_path: Option<String>,
    /// Last converged right-pane body text width (for find footer + tab match counts).
    pub right_pane_text_w: Option<u16>,
    /// Marquee for the left category list in Duplicates / Lenses when the selected name overflows.
    pub category_marquee: ContentMarqueeState,
    /// Marquee for the middle contents path list when the selected row overflows (Snapshot / Delta / Duplicates / Lenses).
    pub content_marquee: ContentMarqueeState,
    /// Zahir compact `columns` table verbosity in Metadata / Writing tabs.
    pub column_stats: crate::config::ColumnStatsDisplay,
}

impl PanelState {
    fn new() -> Self {
        let mut p = Self {
            highlight_style: style::list_highlight(),
            ..Default::default()
        };
        p.category_state.select(Some(0));
        p.content_state.select(Some(0));
        p
    }
}

/// Middle-pane sort direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl SortDirection {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

/// Snapshot/Duplicates middle-pane sort key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnapshotSortKey {
    #[default]
    Name,
    Size,
    Mod,
}

impl SnapshotSortKey {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Mod,
            Self::Mod => Self::Name,
        }
    }
}

/// Mode-aware content sort state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentSort {
    pub snapshot_key: SnapshotSortKey,
    pub snapshot_dir: SortDirection,
    pub delta_dir: SortDirection,
}

impl ContentSort {
    #[must_use]
    pub fn cycle_for_mode(self, main_mode: MainMode) -> Self {
        match main_mode {
            MainMode::Snapshot | MainMode::Duplicates => {
                if self.snapshot_dir == SortDirection::Asc {
                    Self {
                        snapshot_dir: SortDirection::Desc,
                        ..self
                    }
                } else {
                    Self {
                        snapshot_key: self.snapshot_key.next(),
                        snapshot_dir: SortDirection::Asc,
                        ..self
                    }
                }
            }
            MainMode::Delta => Self {
                delta_dir: self.delta_dir.next(),
                ..self
            },
            MainMode::Lenses | MainMode::Settings => self,
        }
    }
}

/// Search bar state.
#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub active: bool,
}

/// In-pane literal search (Shift+S): query, match byte ranges in haystack, current match index.
#[derive(Default)]
pub struct ViewerFindState {
    pub query: String,
    /// Typing into the find bar (chars go to query).
    pub active: bool,
    /// Enter pressed: bar closed, `n` / `N` cycle matches.
    pub committed: bool,
    pub ranges: Vec<(usize, usize)>,
    pub current: usize,
    /// Fingerprint of `(query, haystack)` last used to build `ranges`.
    pub last_sync_token: Option<u64>,
    /// After `n` / `N`, scroll even when the haystack token is unchanged.
    pub pending_scroll: bool,
}

/// Theme selector and override.
#[derive(Default)]
pub struct ThemeState {
    pub selector_visible: bool,
    pub selector_index: usize,
    pub before_selector: Option<String>,
    pub override_name: Option<String>,
}

/// Toast notifications stack and per-operation consumed counts.
#[derive(Default)]
pub struct ToastState {
    pub slots: Vec<ToastSlot>,
    pub consumed_per_operation: HashMap<String, usize>,
}

/// Open (Terminal/GUI) menu state.
#[derive(Default)]
pub struct OpenMenuState {
    pub visible: bool,
    pub path: Option<String>,
    pub can_terminal: bool,
    pub selected_index: usize,
}

/// Lens menu (Add to lens) state.
#[derive(Default)]
pub struct LensMenuState {
    pub visible: bool,
    /// Relative paths to add (one from the quick actions menu (spacebar), many from multi-select bulk).
    pub paths: Vec<String>,
    /// Omit from the picker (Lenses tab: **Add to other lens** must not list the active lens).
    pub exclude_lens_name: Option<String>,
    pub selected_index: usize,
    pub name_input: Option<String>,
}

/// Quick Actions context menu state.
#[derive(Default)]
pub struct QAMenuState {
    pub visible: bool,
    pub selected_index: usize,
    pub kind: Option<SpaceMenuKind>,
}

/// After Space → Enhance policy: choose auto / manual batch Zahir for this directory subtree (local TOML).
#[derive(Default)]
pub struct EnhancePolicyMenuState {
    pub visible: bool,
    pub path: Option<String>,
    pub selected_index: usize,
}

/// Lens rename input and delete-lens confirmation.
#[derive(Default)]
pub struct LensConfirmState {
    pub rename_input: Option<(String, String)>,
    pub delete_visible: bool,
    pub delete_lens_name: Option<String>,
    pub delete_selected: usize,
}

/// Confirm delete for a snapshot file or folder (Yes / No).
#[derive(Default)]
pub struct FileDeleteConfirmState {
    pub visible: bool,
    pub rel_path: Option<String>,
    /// When set, confirm bulk delete (`rel_path` is ignored).
    pub bulk_paths: Option<Vec<String>>,
    pub selected_index: usize,
}

/// Multi-select in the middle pane (Ctrl+Space on contents; cleared when focus leaves the contents list).
#[derive(Debug, Default)]
pub struct MultiselectState {
    pub active: bool,
    pub selected: HashSet<String>,
    pub bulk_menu_visible: bool,
    pub bulk_menu_selected: usize,
    /// When true, bulk menu has a fourth row: Enhance with `ZahirScan` (z). Set when opening the menu.
    pub bulk_menu_zahir_row: bool,
}

impl MultiselectState {
    pub fn clear(&mut self) {
        self.active = false;
        self.selected.clear();
        self.bulk_menu_visible = false;
        self.bulk_menu_selected = 0;
        self.bulk_menu_zahir_row = false;
    }
}

/// After **Ctrl+A**, wait for a letter or show the Command Mode menu (see [`crate::ui::ctrl_chord`]).
#[derive(Clone, Debug, Default)]
pub struct CtrlChordState {
    pub pending: bool,
    pub menu_visible: bool,
    pub started: Option<std::time::Instant>,
}

impl CtrlChordState {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.pending || self.menu_visible
    }
}

/// Command Mode + `p`: pick another indexed root (re-exec `ublx` on that directory).
#[derive(Default)]
pub struct UblxSwitchPickerState {
    pub visible: bool,
    pub selected_index: usize,
    pub roots: Vec<PathBuf>,
}

/// Help overlay and fullscreen right-pane preview.
#[derive(Default)]
pub struct ViewerChrome {
    pub help_visible: bool,
    /// Section tab index inside the help overlay (`Tab` / Shift+Tab); reset when opening help.
    pub help_tab: u8,
    pub viewer_fullscreen: bool,
    pub ctrl_chord: CtrlChordState,
    pub ublx_switch: UblxSwitchPickerState,
}

/// First-run flow when the per-root DB was new: pick root, optional prior roots, then prior-settings or enhance-all.
#[derive(Debug, Clone)]
pub struct StartupPromptState {
    pub phase: StartupPromptPhase,
}

#[derive(Debug, Clone)]
pub enum StartupPromptPhase {
    /// Welcome + root picker: current dir first, then optional recent roots. See [`crate::render::overlays::popup::render_startup_welcome_root_choice`].
    RootChoice {
        selected_index: usize,
        roots: Vec<PathBuf>,
    },
    /// Prior settings for this folder: local `ublx.toml` / cache vs start clean. See [`crate::render::overlays::popup::render_startup_previous_settings_prompt`].
    /// 0 = use saved (copy cache → local when there is no local file), 1 = start fresh.
    PreviousSettings { selected_index: usize },
    /// Enable full-directory `ZahirScan` (`enable_enhance_all`). See [`crate::render::overlays::popup::render_startup_enhance_all_prompt`]. 0 = Yes, 1 = No.
    Enhance { selected_index: usize },
}

/// Background snapshot: user request, poll `.ublx_tmp` while running, and completion.
#[derive(Default)]
pub struct BackgroundSnapshot {
    pub requested: bool,
    pub poll_deadline: Option<std::time::Instant>,
    pub done_received: bool,
    /// After the in-flight snapshot finishes, run one more (e.g. `[[enhance_policy]]` = auto just saved).
    pub defer_snapshot_after_current: bool,
}

/// Lazy-load duplicate groups when the user opens the Duplicates tab.
#[derive(Default)]
pub struct DuplicateLoadGate {
    pub requested: bool,
}

/// Background flat Zahir JSON export (Command Mode + `x`).
#[derive(Default)]
pub struct ZahirExportGate {
    pub requested: bool,
}

/// Background lens Markdown export (Command Mode + `l`).
#[derive(Default)]
pub struct LensExportGate {
    pub requested: bool,
}

/// First real frame vs later ticks; redraw after returning from external editor.
#[derive(Clone, Copy, Debug)]
pub struct SessionTickFlags {
    pub first_tick: bool,
    pub refresh_terminal_after_editor: bool,
}

impl Default for SessionTickFlags {
    fn default() -> Self {
        Self {
            first_tick: true,
            refresh_terminal_after_editor: false,
        }
    }
}

/// Snapshot table reload and one-shot dedup for the background full-enhance toast.
#[derive(Clone, Copy, Debug, Default)]
pub struct SessionReloadFlags {
    /// After single-file `ZahirScan` enhance, reload snapshot rows from DB on next tick.
    pub snapshot_rows: bool,
    /// After we show the "enhancing in background" toast for [`crate::engine::orchestrator::should_force_full_zahir`], suppress duplicates until restart.
    pub force_full_enhance_toast_shown: bool,
    /// After deleting a file from Duplicates mode, reload duplicate groups from the DB on next tick.
    pub duplicate_groups: bool,
}

/// One-shot session coordination for ticks, editor handoff, and DB reload.
#[derive(Default)]
pub struct SessionFlow {
    pub tick: SessionTickFlags,
    pub reload: SessionReloadFlags,
    /// Set when the user confirms another indexed root in the project picker; next tick runs [`crate::handlers::session_switch::perform_session_switch`].
    pub pending_switch_to: Option<PathBuf>,
}

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

    /// Clear loaded image, error, and async decode channel; **retains** [`Self::picker`] so the
    /// terminal is not re-queried on every selection (matches previous flat-field behavior).
    /// Finished previews are moved into [`Self::image_lru`] so returning to an image can be instant.
    pub fn clear(&mut self) {
        self.pdf.prefetch_cancel.fetch_add(1, Ordering::SeqCst);
        self.pdf.prefetch_rx = None;
        self.pdf.prefetch_earliest = None;
        self.decode_rx = None;
        self.pdf.page_count_rx = None;
        self.err = None;
        let k = self.key.take();
        let p = self.protocol.take();
        if let (Some(k), Some(p)) = (k, p) {
            self.push_lru(k, p);
        }
        self.pdf.for_path = None;
        self.pdf.page = 1;
        self.pdf.page_count = None;
    }
}

/// Avoids re-reading the selected file every UI tick when path, category, size, and mtime match.
#[derive(Debug, Clone)]
pub struct ViewerDiskContentCache {
    pub rel_path: String,
    /// Snapshot category (drives file-type handling in the viewer).
    pub category: String,
    pub file_len: u64,
    pub modified: Option<std::time::SystemTime>,
    pub viewer_str: Option<String>,
    pub embedded_cover_raster: Option<Vec<u8>>,
    pub viewer_can_open: bool,
}

impl ViewerDiskContentCache {
    #[must_use]
    pub fn matches(&self, path: &str, category: &str, meta: &std::fs::Metadata) -> bool {
        self.rel_path == path
            && self.category == category
            && self.file_len == meta.len()
            && self.modified == meta.modified().ok()
    }
}

#[derive(Default)]
pub struct RightPaneAsync {
    pub generation: u64,
    pub last_spawn_path: String,
    pub displayed: RightPaneContent,
    pub rx: Option<tokio::sync::mpsc::UnboundedReceiver<RightPaneAsyncReady>>,
}

/// Top-level TUI state. Menu and UI sub-states are grouped into nested structs.
pub struct UblxState {
    pub main_mode: MainMode,
    pub right_pane_mode: RightPaneMode,
    pub panels: PanelState,
    pub search: SearchState,
    pub viewer_find: ViewerFindState,
    pub theme: ThemeState,
    pub toasts: ToastState,
    pub open_menu: OpenMenuState,
    pub lens_menu: LensMenuState,
    pub qa_menu: QAMenuState,
    pub enhance_policy_menu: EnhancePolicyMenuState,
    pub lens_confirm: LensConfirmState,
    /// Rename entry: `(relative path, new basename being typed)`.
    pub file_rename_input: Option<(String, String)>,
    pub file_delete_confirm: FileDeleteConfirmState,
    pub multiselect: MultiselectState,
    pub chrome: ViewerChrome,
    pub cached_tree: Option<(String, String)>,
    /// Same file row as last tick: reuse viewer text / cover bytes without disk reads.
    pub viewer_disk_cache: Option<ViewerDiskContentCache>,
    /// Viewer: large markdown only — cached styled [`Text`] + viewport slice on scroll.
    pub viewer_text_cache: Option<cache::ViewerTextCacheEntry>,
    /// Last rendered preview fingerprint for invalidating viewer caches when path or buffer identity changes.
    pub viewer_preview_source: Option<(String, cache::ViewerContentIdentity)>,
    /// Viewer: up to [`crate::engine::cache::VIEWER_TEXT_CACHE`] `csv_lru_cap` delimiter-table `Text` bodies by path/width/theme/revision.
    pub csv_table_text_lru:
        cache::LruCache<cache::ViewerTableCacheKey, cache::ViewerTextCacheEntry>,
    /// Large markdown / syntect / CSV table builds off the UI thread ([`crate::render::viewers::async_render`]).
    pub viewer_async: ViewerAsyncState,
    /// Image category viewer ([`RightPaneContent::derived`] `abs_path` + [`crate::render::viewers::image`]).
    pub viewer_image: ViewerImageState,
    pub last_key_for_double: Option<char>,
    pub snapshot_bg: BackgroundSnapshot,
    pub duplicate_load: DuplicateLoadGate,
    pub zahir_export_load: ZahirExportGate,
    pub lens_export_load: LensExportGate,
    /// Duplicates tab: paths hidden for this session via Space → Ignore (i); not persisted.
    pub duplicate_ignored_paths: HashSet<String>,
    pub config_written_by_us_at: Option<std::time::Instant>,
    pub session: SessionFlow,
    /// CLI to pipe UTF-8 into for clipboard (see [`ClipboardCopyCommand::detect`]); None if nothing found.
    pub clipboard_copy: Option<ClipboardCopyCommand>,
    /// Shown when the per-root DB file under `ubli/` was new this run ([`crate::config::paths::should_show_initial_prompt`]).
    pub startup_prompt: Option<StartupPromptState>,
    pub settings: SettingsPaneState,
    pub right_pane_async: RightPaneAsync,
}

impl Default for UblxState {
    fn default() -> Self {
        Self::new()
    }
}

impl UblxState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            main_mode: MainMode::default(),
            right_pane_mode: RightPaneMode::default(),
            panels: PanelState::new(),
            search: SearchState::default(),
            viewer_find: ViewerFindState::default(),
            theme: ThemeState::default(),
            toasts: ToastState::default(),
            open_menu: OpenMenuState::default(),
            lens_menu: LensMenuState::default(),
            qa_menu: QAMenuState::default(),
            enhance_policy_menu: EnhancePolicyMenuState::default(),
            lens_confirm: LensConfirmState::default(),
            file_rename_input: None,
            file_delete_confirm: FileDeleteConfirmState::default(),
            multiselect: MultiselectState::default(),
            chrome: ViewerChrome::default(),
            cached_tree: None,
            viewer_disk_cache: None,
            viewer_text_cache: None,
            viewer_preview_source: None,
            csv_table_text_lru: cache::LruCache::default(),
            viewer_async: ViewerAsyncState::default(),
            viewer_image: ViewerImageState::default(),
            last_key_for_double: None,
            snapshot_bg: BackgroundSnapshot::default(),
            duplicate_load: DuplicateLoadGate::default(),
            zahir_export_load: ZahirExportGate::default(),
            lens_export_load: LensExportGate::default(),
            duplicate_ignored_paths: HashSet::new(),
            config_written_by_us_at: None,
            session: SessionFlow::default(),
            clipboard_copy: ClipboardCopyCommand::detect(),
            startup_prompt: None,
            settings: SettingsPaneState::default(),
            right_pane_async: RightPaneAsync::default(),
        }
    }

    /// Reset open menu state (Esc or after action).
    pub fn close_open_menu(&mut self) {
        self.open_menu.visible = false;
        self.open_menu.path = None;
        self.open_menu.can_terminal = false;
    }

    /// Open the Open (Terminal/GUI) menu. When `can_open_in_terminal` is true, show both options; otherwise only Open (GUI).
    pub fn open_open_menu(&mut self, path: String, can_open_in_terminal: bool) {
        self.open_menu.visible = true;
        self.open_menu.path = Some(path);
        self.open_menu.can_terminal = can_open_in_terminal;
        self.open_menu.selected_index = 0;
    }

    /// Reset lens menu state (Esc or after adding to lens). Does not clear [`LensMenuState::name_input`].
    pub fn close_lens_menu(&mut self) {
        self.lens_menu.visible = false;
        self.lens_menu.paths.clear();
        self.lens_menu.selected_index = 0;
    }

    /// Reset spacebar context menu state.
    pub fn close_qa_menu(&mut self) {
        self.qa_menu.visible = false;
        self.qa_menu.selected_index = 0;
        self.qa_menu.kind = None;
    }

    pub fn close_enhance_policy_menu(&mut self) {
        self.enhance_policy_menu.visible = false;
        self.enhance_policy_menu.path = None;
        self.enhance_policy_menu.selected_index = 0;
    }

    /// Reset delete-lens confirmation popup state.
    pub fn close_lens_delete_confirm(&mut self) {
        self.lens_confirm.delete_visible = false;
        self.lens_confirm.delete_lens_name = None;
        self.lens_confirm.delete_selected = 0;
    }

    /// Open the Lens menu (Add to lens) for the given relative path(s).
    /// `exclude_current_lens`: lens name to omit from the list (e.g. active lens on Lenses tab).
    pub fn open_lens_menu(&mut self, paths: Vec<String>, exclude_current_lens: Option<String>) {
        if paths.is_empty() {
            return;
        }
        self.lens_menu.visible = true;
        self.lens_menu.paths = paths;
        self.lens_menu.exclude_lens_name = exclude_current_lens;
        self.lens_menu.selected_index = 0;
    }

    /// Open the spacebar context menu with the given kind.
    pub fn open_qa_menu(&mut self, kind: SpaceMenuKind) {
        self.qa_menu.visible = true;
        self.qa_menu.selected_index = 0;
        self.qa_menu.kind = Some(kind);
    }

    /// Show the delete-lens confirmation for the given lens name.
    pub fn open_lens_delete_confirm(&mut self, lens_name: String) {
        self.lens_confirm.delete_visible = true;
        self.lens_confirm.delete_lens_name = Some(lens_name);
        self.lens_confirm.delete_selected = 0;
    }

    /// quick actions menu (spacebar) → Rename: centered text input with current basename.
    pub fn open_file_rename_input(&mut self, rel_path: String) {
        let base = std::path::Path::new(&rel_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.file_rename_input = Some((rel_path, base));
    }

    pub fn close_file_delete_confirm(&mut self) {
        self.file_delete_confirm.visible = false;
        self.file_delete_confirm.rel_path = None;
        self.file_delete_confirm.bulk_paths = None;
        self.file_delete_confirm.selected_index = 0;
    }

    pub fn open_file_delete_confirm(&mut self, rel_path: String) {
        self.file_delete_confirm.visible = true;
        self.file_delete_confirm.rel_path = Some(rel_path);
        self.file_delete_confirm.bulk_paths = None;
        self.file_delete_confirm.selected_index = 0;
    }

    pub fn open_file_delete_confirm_bulk(&mut self, paths: Vec<String>) {
        self.file_delete_confirm.visible = true;
        self.file_delete_confirm.rel_path = None;
        self.file_delete_confirm.bulk_paths = Some(paths);
        self.file_delete_confirm.selected_index = 0;
    }
}

/// Which config file the Settings tab edits (`~/.config/ublx/ublx.toml` vs project `ublx.toml`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsConfigScope {
    #[default]
    Global,
    Local,
}

/// Settings tab: bool/layout editor, raw TOML preview scroll, and path to the file being edited.
#[derive(Clone, Debug)]
pub struct SettingsPaneState {
    pub scope: SettingsConfigScope,
    /// Focus row on the left: bool indices, then layout button, then three layout fields when unlocked.
    pub left_cursor: usize,
    pub right_scroll: u16,
    pub layout_unlocked: bool,
    pub layout_left_buf: String,
    pub layout_mid_buf: String,
    pub layout_right_buf: String,
    pub opacity_unlocked: bool,
    pub opacity_buf: String,
    /// Resolved path for the active scope (refreshed on enter / scope change).
    pub editing_path: Option<std::path::PathBuf>,
}

impl Default for SettingsPaneState {
    fn default() -> Self {
        Self {
            scope: SettingsConfigScope::Global,
            left_cursor: 0,
            right_scroll: 0,
            layout_unlocked: false,
            layout_left_buf: String::new(),
            layout_mid_buf: String::new(),
            layout_right_buf: String::new(),
            opacity_unlocked: false,
            opacity_buf: String::new(),
            editing_path: None,
        }
    }
}

/// Top-level mode. Tab bar order: Snapshot, Lenses (optional), Delta, Duplicates (optional), Settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MainMode {
    #[default]
    Snapshot,
    Delta,
    /// Single-pane config editor (global / local `ublx.toml`).
    Settings,
    Duplicates,
    Lenses,
}

impl MainMode {
    /// Cycle Snapshot → Lenses (if any) → Delta → Duplicates (if any) → Settings → Snapshot (`MainModeToggle` / `~`).
    #[must_use]
    pub fn next(self, has_duplicates: bool, has_lenses: bool) -> MainMode {
        match self {
            MainMode::Snapshot => {
                if has_lenses {
                    MainMode::Lenses
                } else {
                    MainMode::Delta
                }
            }
            MainMode::Lenses => MainMode::Delta,
            MainMode::Delta => {
                if has_duplicates {
                    MainMode::Duplicates
                } else {
                    MainMode::Settings
                }
            }
            MainMode::Duplicates => MainMode::Settings,
            MainMode::Settings => MainMode::Snapshot,
        }
    }
}

/// Which panel has focus (Categories or Contents; Metadata is read-only).
#[derive(Clone, Copy, Default, PartialEq)]
pub enum PanelFocus {
    #[default]
    Categories,
    Contents,
}

/// Which variant of the spacebar context menu is open (determines items and Enter behavior).
#[derive(Clone, Debug)]
pub enum SpaceMenuKind {
    /// File actions for a selected file path (relative): Open, Show in folder, optional enhance,
    /// Add to lens or delete from current lens (Lenses tab uses d), Copy Path, optional Copy Templates, Rename, Delete.
    /// `can_open_in_terminal`: when true, Open shows Terminal+GUI; else GUI only.
    FileActions {
        path: String,
        can_open_in_terminal: bool,
        /// Show subtree batch-enhance policy when the snapshot row is [`CATEGORY_DIRECTORY`].
        show_enhance_directory_policy: bool,
        /// Show "Enhance with `ZahirScan`" when [`crate::config::UblxOpts::enable_enhance_all`] is false and row has no `zahir_json`.
        show_enhance_zahir: bool,
        /// Show "Copy Zahir JSON" when this row has non-empty `zahir_json` in the snapshot (copies raw JSON to clipboard).
        show_copy_zahir_json: bool,
    },
    /// Lens panel actions: `lens_name` is the selected lens. Options: Rename, Delete.
    LensPanelActions { lens_name: String },
    /// Duplicates tab only: hide path from duplicate lists for this session, or delete the file.
    DuplicateMemberActions { path: String },
}

/// Per-pane content from zahir JSON. Templates always present; metadata and writing only if keys exist.
pub struct SectionedPreview {
    pub templates: String,
    pub metadata: Option<String>,
    pub writing: Option<String>,
}

/// Snapshot mode: indices into the single in-memory list (no copy). Delta mode: small owned vec.
#[derive(Clone)]
pub enum ViewContents {
    /// Indices into the caller's `all_rows` slice (snapshot mode — one copy of list).
    SnapshotIndices(Vec<usize>),
    /// Owned rows for delta mode (added/mod/removed paths; typically small).
    DeltaRows(Vec<TuiRow>),
}

/// Derived list data for this tick: filtered categories and contents (by index or owned), lengths for navigation.
/// Scalability: snapshot mode uses [`ViewContents::SnapshotIndices`] so we keep a single copy of the list; no cloned row vec.
pub struct ViewData {
    pub filtered_categories: Vec<String>,
    pub contents: ViewContents,
    pub category_list_len: usize,
    pub content_len: usize,
}

impl ViewData {
    /// Row at content index `i`. For [`ViewContents::SnapshotIndices`], pass `Some(all_rows)`; for [`ViewContents::DeltaRows`], pass `None`.
    #[must_use]
    pub fn row_at<'a>(&'a self, i: usize, all_rows: Option<&'a [TuiRow]>) -> Option<&'a TuiRow> {
        match &self.contents {
            ViewContents::SnapshotIndices(indices) => indices
                .get(i)
                .and_then(|&pos| all_rows.and_then(|r| r.get(pos))),
            ViewContents::DeltaRows(rows) => rows.get(i),
        }
    }

    /// Iterate over content rows. For [`ViewContents::SnapshotIndices`], pass `Some(all_rows)`; for [`ViewContents::DeltaRows`], pass `None`.
    #[must_use]
    pub fn iter_contents<'a>(
        &'a self,
        all_rows: Option<&'a [TuiRow]>,
    ) -> Box<dyn Iterator<Item = &'a TuiRow> + 'a> {
        match &self.contents {
            ViewContents::SnapshotIndices(indices) => {
                let iter = indices
                    .iter()
                    .filter_map(move |&pos| all_rows.and_then(|r| r.get(pos)));
                Box::new(iter)
            }
            ViewContents::DeltaRows(rows) => Box::new(rows.iter()),
        }
    }
}

/// Raw delta row: (`created_ns`, path) from `delta_log`. Used to build display lines with dates preserved when filtering.
pub type DeltaRow = (i64, String);

/// Data for Delta mode: snapshot overview text and raw (`created_ns`, path) rows per delta type.
pub struct DeltaViewData {
    pub overview_text: String,
    pub added_rows: Vec<DeltaRow>,
    pub mod_rows: Vec<DeltaRow>,
    pub removed_rows: Vec<DeltaRow>,
}

impl DeltaViewData {
    /// Raw rows for the given category index. Uses [`DeltaType::from_index`].
    #[must_use]
    pub fn rows_by_index(&self, idx: usize) -> &[DeltaRow] {
        match DeltaType::from_index(idx) {
            DeltaType::Added => &self.added_rows,
            DeltaType::Mod => &self.mod_rows,
            DeltaType::Removed => &self.removed_rows,
        }
    }
}

/// Result from background right-pane resolve.
#[derive(Debug)]
pub struct RightPaneAsyncReady {
    pub generation: u64,
    pub path: String,
    pub content: RightPaneContent,
    pub disk_cache: Option<ViewerDiskContentCache>,
}

#[derive(Clone, Debug, Default)]
pub struct SnapshotEntryMeta {
    pub path: Option<String>,
    pub category: Option<String>,
    pub size: Option<u64>,
    pub mtime_ns: Option<i64>,
    pub has_zahir_json: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RightPaneContentDerived {
    pub abs_path: Option<PathBuf>,
    pub can_open: bool,
    pub offer_enhance_zahir: bool,
    pub offer_enhance_directory_policy: bool,
    pub embedded_cover_raster: Option<Vec<u8>>,
}

/// Text to show in the right pane for the current selection.
#[derive(Default, Clone, Debug)]
pub struct RightPaneContent {
    pub templates: String,
    pub metadata: Option<String>,
    pub writing: Option<String>,
    /// File/tree preview body; shared by reference for async highlight jobs (cheap `Arc::clone`).
    pub viewer: Option<Arc<str>>,
    /// When set with a directory tree [`viewer`] body, shown above the tree (bold + italic in the UI).
    pub viewer_directory_policy_line: Option<String>,
    pub snap_meta: SnapshotEntryMeta,
    pub derived: RightPaneContentDerived,
}

impl RightPaneContent {
    /// Empty right-pane content (e.g. Delta mode has no selection-based viewer).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Zahir / viewer routing type from snapshot `category` (see [`crate::integrations::file_type_from_metadata_name`]).
    #[must_use]
    pub fn zahir_file_type(&self) -> Option<ZahirFT> {
        file_type_from_metadata_name(self.snap_meta.category.as_deref().unwrap_or(""))
    }

    /// Snapshot `category` column as [`UblxDbCategory`] (same classification as the DB / [`UblxDbCategory::get_category_for_path`]).
    #[must_use]
    pub fn ublx_db_category(&self) -> UblxDbCategory {
        UblxDbCategory::from_snapshot_category(self.snap_meta.category.as_deref().unwrap_or(""))
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum RightPaneMode {
    #[default]
    Viewer,
    Templates,
    Metadata,
    Writing,
}
