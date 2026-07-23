//! Top-level [`UblxState`] aggregate.

use std::collections::HashSet;

use crate::engine::{cache, viewer_async::ViewerAsyncState};
use crate::utils::ClipboardCopyCommand;

use super::modes::{MainMode, RightPaneMode, SpaceMenuKind};
use super::overlays::{
    EnhancePolicyMenuState, FileDeleteConfirmState, LensConfirmState, LensMenuState, MultiselectState,
    OpenMenuState, QAMenuState, SearchState, StartupPromptState, ThemeState, ToastState,
    ViewerChrome, ViewerFindState,
};
use super::panels::PanelState;
use super::session::{
    BackgroundSnapshot, DuplicateLoadGate, LensExportGate, SessionFlow, ZahirExportGate,
};
use super::settings::SettingsPaneState;
use super::view::{RightPaneAsync, ViewerDiskContentCache};
use super::viewer::ViewerImageState;

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
