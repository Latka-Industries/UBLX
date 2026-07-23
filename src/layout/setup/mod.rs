//! 3-panel TUI: categories (left), contents (middle), preview (right).
//!
//! [`crate::handlers::core::run_tui_session`] drives the loop; work per tick is split into four phases (see classification below).
//! Action application (key → state changes) lives in [`crate::handlers::state_transitions`].

mod modes;
mod overlays;
mod panels;
mod session;
mod settings;
mod ublx_state;
mod view;
mod viewer;

/// Re-export snapshot row type for layout/view/render (`path`, category, size).
pub use crate::engine::db_ops::SnapshotTuiRow as TuiRow;

/// Category string for directories in the snapshot (matches [`crate::engine::db_ops::UblxDbCategory`]).
pub const CATEGORY_DIRECTORY: &str = "Directory";

pub use modes::{MainMode, PanelFocus, RightPaneMode, SettingsConfigScope, SpaceMenuKind};
pub use overlays::{
    CtrlChordState, EnhancePolicyMenuState, FileDeleteConfirmState, LensConfirmState,
    LensMenuState, MultiselectState, OpenMenuState, QAMenuState, SearchState, StartupPromptPhase,
    StartupPromptState, ThemeState, ToastState, UblxSwitchPickerState, ViewerChrome,
    ViewerFindState,
};
pub use panels::{ContentMarqueeState, ContentSort, PanelState, SnapshotSortKey, SortDirection};
pub use session::{
    BackgroundSnapshot, DuplicateLoadGate, LensExportGate, SessionFlow, SessionReloadFlags,
    SessionTickFlags, ZahirExportGate,
};
pub use settings::SettingsPaneState;
pub use ublx_state::UblxState;
pub use view::{
    DeltaRow, DeltaViewData, RightPaneAsync, RightPaneAsyncReady, RightPaneContent,
    RightPaneContentDerived, SectionedPreview, SnapshotEntryMeta, ViewContents, ViewData,
    ViewerDiskContentCache,
};
pub use viewer::{PDF, ViewerImageState};
