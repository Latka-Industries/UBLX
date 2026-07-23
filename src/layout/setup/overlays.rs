//! Overlay, menu, and chrome UI state.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::utils::ToastSlot;

use super::modes::SpaceMenuKind;

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
