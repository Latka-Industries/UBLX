//! Top-level and right-pane mode enums.

/// Which config file the Settings tab edits (`~/.config/ublx/ublx.toml` vs project `ublx.toml`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsConfigScope {
    #[default]
    Global,
    Local,
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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum RightPaneMode {
    #[default]
    Viewer,
    Templates,
    Metadata,
    Writing,
}
