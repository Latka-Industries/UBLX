//! Settings tab pane state.

use super::modes::SettingsConfigScope;

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
