//! User-visible string tables and `UiStrings` helpers.

use crate::utils::StringObjTraits;

use super::glyph::UI_GLYPHS;
use super::tabs::UiStringsMainTabs;

/// Generic and feature-specific loading lines.
pub struct UiStringsMisc {
    /// Short spinner / placeholder (e.g. delta pane while data loads).
    pub loading: &'static str,
    pub json_items: &'static str,
}

/// Delta mode: section titles and row labels.
pub struct UiStringsDelta {
    pub added: &'static str,
    pub modified: &'static str,
    pub removed: &'static str,
    pub right_title: &'static str,
    /// Left pane block title (delta list).
    pub left_block_title: &'static str,
    pub placeholder_dash: &'static str,
    pub type_label: &'static str,
}

/// Snapshot / viewer pane titles and tab labels.
pub struct UiStringsPane {
    pub categories: &'static str,
    pub contents: &'static str,
    pub viewer: &'static str,
    pub templates: &'static str,
    pub metadata: &'static str,
    pub writing: &'static str,
    pub tab_templates: &'static str,
    pub tab_viewer: &'static str,
    pub tab_metadata: &'static str,
    pub tab_writing: &'static str,
    pub not_available: &'static str,
    pub viewer_placeholder: &'static str,
    /// Line above `tree` output for directory rows: label in `"{label}: {value}"`.
    pub current_enhance_policy_label: &'static str,
    pub directory_policy_auto: &'static str,
    pub directory_policy_manual: &'static str,
    pub directory_policy_inherit_auto: &'static str,
    pub directory_policy_inherit_manual: &'static str,
}

/// Middle / list column (All, empty states, row prefix).
pub struct UiStringsList {
    pub all_categories: &'static str,
    pub no_contents: &'static str,
    pub no_matches: &'static str,
    pub list_symbol: &'static str,
}

/// Status / search line (snapshot + query).
pub struct UiStringsSearchStatus {
    pub search_label: &'static str,
    pub find_label: &'static str,
    pub last_snapshot: &'static str,
}

/// UBLX settings source labels (global vs local config).
pub struct UiStringsConfig {
    pub global: &'static str,
    pub local: &'static str,
}

/// Paths column and group labels (duplicates / lenses).
pub struct UiStringsPaths {
    pub paths: &'static str,
    pub duplicate_group: &'static str,
    pub lens_group: &'static str,
}

pub struct UiStringsBrand {
    pub brand: &'static str,
}

pub struct UiStringsTables {
    pub header_key: &'static str,
    pub header_value: &'static str,
    pub first_title: &'static str,
    pub contents_title: &'static str,
    pub columns_title: &'static str,
}

/// Modal / overlay titles and table column headers for help.
pub struct UiStringsDialogs {
    pub help: &'static str,
    pub theme: &'static str,
    pub notification: &'static str,
    /// Command Mode popup (`Ctrl+A`); use with [`UiStrings::pad`].
    pub command_mode_popup: &'static str,
    /// First column header in the Command Mode table (single-letter key after Ctrl+A).
    pub command_mode_key_column: &'static str,
    /// Bulk actions popup (multi-select).
    pub multiselect_bulk_title: &'static str,
    /// Help overlay section for multi-select.
    pub multiselect_help_title: &'static str,
    /// Help overlay: section title above Viewer pane shortcuts.
    pub help_section_viewer: &'static str,
    /// Help overlay: section title above quick actions menu (spacebar) shortcuts.
    pub help_section_qa: &'static str,
    pub help_command: &'static str,
    pub help_action: &'static str,
    /// [`crate::render::overlays::popup::render_ublx_switch_picker`] border title; use with [`UiStrings::pad`].
    pub ublx_switch_popup: &'static str,
    /// [`crate::render::overlays::popup::render_ublx_switch_picker`]: table header (indexed root path).
    pub ublx_switch_column_path: &'static str,
    /// No recents entries with a DB — body line in the switch picker.
    pub ublx_switch_empty: &'static str,
}

/// Image / PDF / raster preview chrome and error prefixes (detail after `: ` from `format!`).
pub struct UiStringsViewerRaster {
    /// PDF footer: `format!("{} {p} / {n}", self.page_label, ...)`.
    pub page_label: &'static str,
    pub could_not_load_preview: &'static str,
    pub could_not_decode_cover: &'static str,
    pub could_not_open_image: &'static str,
}

pub struct UiStringsToasts {
    pub config_reloaded: &'static str,
    pub no_duplicates: &'static str,
    /// Toast when a full Zahir snapshot starts after `enable_enhance_all` was off in the cached overlay and is now on (Command Mode snapshot or first tick).
    pub force_full_enhance_background: &'static str,
    pub enhanced_with_zahirscan: &'static str,
    pub enhance_failed_prefix: &'static str,
    pub copied_path_to_clipboard: &'static str,
    pub copied_zahir_json_to_clipboard: &'static str,
    pub copy_path_failed_prefix: &'static str,
    pub copy_zahir_json_failed_prefix: &'static str,
    /// Placeholder `{LENS}` replaced with the lens name.
    pub removed_from_lens: &'static str,
    /// Replace `{PATH}` with the new relative path after rename.
    pub file_renamed: &'static str,
    pub file_deleted: &'static str,
    pub file_ops_failed_prefix: &'static str,
    pub multiselect_none_selected: &'static str,
    /// Replace `{N}` with the number of files renamed.
    pub bulk_renamed_n: &'static str,
    pub bulk_rename_no_editor: &'static str,
    pub bulk_rename_editor_failed: &'static str,
    pub bulk_rename_no_changes: &'static str,
    /// Replace `{N}` with count and `{LENS}` with lens name (bulk remove from lens).
    pub bulk_removed_n_from_lens: &'static str,
    /// Replace `{N}` with count (multi-select bulk Enhance with `ZahirScan`).
    pub bulk_enhanced_zahir_n: &'static str,
    /// Duplicates tab: Space → Ignore (i); path hidden until reload or session end.
    pub duplicate_member_ignored: &'static str,
    /// Replace `{N}` with file count (Command Mode export to `ublx-export/`).
    pub export_zahir_ok: &'static str,
    pub export_zahir_none: &'static str,
    pub export_zahir_failed_prefix: &'static str,
    /// Replace `{N}` with lens file count (Command Mode export to `ublx-lenses/`).
    pub export_lenses_ok: &'static str,
    pub export_lenses_none: &'static str,
    pub export_lenses_failed_prefix: &'static str,
}

pub struct UiStringsLens {
    pub menu_create_new: &'static str,
    pub name_prompt: &'static str,
    pub rename_prompt: &'static str,
    pub delete_confirm_title: &'static str,
    pub delete_yes: &'static str,
    pub delete_no: &'static str,
    /// Replace `{LENS}` with the lens name (same pattern as other lens toasts).
    pub toast_created_and_added_file: &'static str,
    pub toast_added_to_lens: &'static str,
    pub toast_renamed_to: &'static str,
    pub toast_deleted_lens: &'static str,
}

/// Rename / delete entry under the indexed root (quick actions menu (spacebar) file actions).
pub struct UiStringsFile {
    pub rename_prompt: &'static str,
    pub delete_confirm_title: &'static str,
}

/// Settings tab: bool row labels (TOML key names).
pub struct UiStringsSettingsBool {
    pub show_hidden_files: &'static str,
    pub hash: &'static str,
    pub enable_enhance_all: &'static str,
    pub ask_enhance_on_new_root: &'static str,
    /// Global / local: spawn background index when opening the TUI (next session if changed here).
    pub run_snapshot_on_startup: &'static str,
    pub unknown_row: &'static str,
}

/// Settings tab left/right panes ([`crate::render::panes::settings_mode`]).
pub struct UiStringsSettingsPane {
    pub global_careful_title: &'static str,
    pub global_careful_detail: &'static str,
    pub typed_column_tables_label: &'static str,
    pub typed_column_tables_none: &'static str,
    pub typed_column_tables_abbrev: &'static str,
    pub typed_column_tables_full: &'static str,
    pub typed_column_tables_footnote: &'static str,
    pub opacity_format_label: &'static str,
    pub rgba_toggle: &'static str,
    pub hex8_toggle: &'static str,
    pub edit_enter_save_lock: &'static str,
    pub edit_enter_unlock: &'static str,
    /// `format!` with one `{}` for the primary hint (`edit_enter_*`).
    pub edit_layout_template: &'static str,
    pub layout_left_pct: &'static str,
    pub layout_middle_pct: &'static str,
    pub layout_right_pct: &'static str,
    /// `format!` with one `{}` for the primary hint.
    pub edit_opacity_template: &'static str,
    pub opacity_value_label: &'static str,
    pub opacity_format_footnote: &'static str,
    pub external_apps_title: &'static str,
    pub ffmpeg_label: &'static str,
    pub tool_available: &'static str,
    pub tool_not_found: &'static str,
    pub pdf_label: &'static str,
    pub pdf_backends_poppler_and_mupdf: &'static str,
    pub pdf_backends_poppler_only: &'static str,
    pub pdf_backends_mupdf_only: &'static str,
    pub snapshot_applied_footnote: &'static str,
    pub yn_yes: &'static str,
    pub yn_no: &'static str,
    pub right_pane_title: &'static str,
    pub path_global_unavailable: &'static str,
    pub path_local_missing: &'static str,
}

/// First launch: no local `ublx.toml` yet.
pub struct UiStringsFirstRun {
    pub welcome_title: &'static str,
    pub root_choice_footer: &'static str,
    pub ublx_here: &'static str,
    pub recent_heading: &'static str,
    pub enhance_prompt_title: &'static str,
    /// Shown below Yes/No (hint style). `ublx.toml` / `.ublx.toml`: `enable_enhance_all`.
    pub enhance_prompt_footnote: &'static str,
    pub enhance_yes: &'static str,
    pub enhance_no: &'static str,
    pub previous_settings_title: &'static str,
    pub previous_settings_footnote: &'static str,
    pub previous_settings_use: &'static str,
    pub previous_settings_fresh: &'static str,
}

pub struct UiStringsSpaceMenu {
    pub open: &'static str,
    /// Reveal in Finder / Explorer, or open parent folder (Linux).
    pub show_in_folder: &'static str,
    pub copy_path: &'static str,
    /// Index-time batch Zahir for this directory subtree (`[[enhance_policy]]`); snapshot Directory rows only.
    pub enhance_policy: &'static str,
    pub enhance_policy_always: &'static str,
    pub enhance_policy_never: &'static str,
    /// Run full `ZahirScan` on this file when `enable_enhance_all` is false.
    pub enhance_with_zahirscan: &'static str,
    /// Copy raw snapshot `zahir_json` for this entry to the clipboard (quick actions menu (spacebar); only when JSON exists).
    pub copy_zahir_json: &'static str,
    pub add_to_lens: &'static str,
    /// Multi-select bulk / lens picker when already viewing a lens: add paths elsewhere.
    pub add_to_other_lens: &'static str,
    /// Label for removing a path from the lens you are viewing (hotkey `d` on Lenses tab).
    pub remove_from_lens: &'static str,
    pub rename: &'static str,
    pub delete: &'static str,
    /// Duplicates tab only: hide this path from duplicate lists for this session (quick actions menu (spacebar)).
    pub ignore_in_duplicates: &'static str,
}

/// All symbols and string literals used by the renderer.
pub struct UiStrings {
    pub misc: UiStringsMisc,
    pub delta: UiStringsDelta,
    pub pane: UiStringsPane,
    pub list: UiStringsList,
    pub main_tabs: UiStringsMainTabs,
    pub search: UiStringsSearchStatus,
    pub config: UiStringsConfig,
    pub paths: UiStringsPaths,
    pub brand: UiStringsBrand,
    pub tables: UiStringsTables,
    pub dialogs: UiStringsDialogs,
    pub viewer_raster: UiStringsViewerRaster,
    pub toasts: UiStringsToasts,
    pub lens: UiStringsLens,
    pub space: UiStringsSpaceMenu,
    pub file: UiStringsFile,
    pub first_run: UiStringsFirstRun,
    pub settings_bool: UiStringsSettingsBool,
    pub settings_pane: UiStringsSettingsPane,
}

impl Default for UiStrings {
    fn default() -> Self {
        Self::new()
    }
}

impl StringObjTraits for UiStrings {
    fn new() -> Self {
        UiStrings::new()
    }
}

impl UiStrings {
    const fn misc() -> UiStringsMisc {
        UiStringsMisc {
            loading: "Loading…",
            json_items: "item(s)",
        }
    }

    const fn delta() -> UiStringsDelta {
        UiStringsDelta {
            added: "Added",
            modified: "Modified",
            removed: "Removed",
            right_title: "Snapshot overview",
            left_block_title: "Delta",
            placeholder_dash: "—",
            type_label: "Delta type",
        }
    }

    const fn pane() -> UiStringsPane {
        UiStringsPane {
            categories: "Categories",
            contents: "Contents",
            viewer: "Viewer",
            templates: "Templates",
            metadata: "Metadata",
            writing: "Writing",
            tab_templates: "Templates (t)",
            tab_viewer: "Viewer (v)",
            tab_metadata: "Metadata (m)",
            tab_writing: "Writing (w)",
            not_available: "(not available for this item)",
            viewer_placeholder: "(viewer — file content will load here)",
            current_enhance_policy_label: "Current enhance policy",
            directory_policy_auto: "Auto",
            directory_policy_manual: "Manual",
            directory_policy_inherit_auto: "Inherit (global auto)",
            directory_policy_inherit_manual: "Inherit (global manual)",
        }
    }

    const fn list() -> UiStringsList {
        UiStringsList {
            all_categories: "All",
            no_contents: "(no contents)",
            no_matches: "(no matches)",
            list_symbol: "  ",
        }
    }

    const fn main_tabs() -> UiStringsMainTabs {
        UiStringsMainTabs {
            snapshot: "Snapshot",
            delta: "Delta",
            settings: "Settings",
            duplicates: "Duplicates",
            lenses: "Lenses",
        }
    }

    const fn search() -> UiStringsSearchStatus {
        UiStringsSearchStatus {
            search_label: "Search (Categories & Contents): ",
            find_label: "Search: ",
            last_snapshot: "Last Snapshot",
        }
    }

    const fn config() -> UiStringsConfig {
        UiStringsConfig {
            global: "Global",
            local: "Local",
        }
    }

    const fn paths() -> UiStringsPaths {
        UiStringsPaths {
            paths: "Paths",
            duplicate_group: "Duplicate",
            lens_group: "Lens",
        }
    }

    const fn brand() -> UiStringsBrand {
        UiStringsBrand { brand: "UBLX" }
    }

    const fn tables() -> UiStringsTables {
        UiStringsTables {
            header_key: "Key",
            header_value: "Value",
            first_title: "General",
            contents_title: "Contents",
            columns_title: "Columns",
        }
    }

    const fn dialogs() -> UiStringsDialogs {
        UiStringsDialogs {
            help: "Help",
            theme: "Theme",
            notification: "Notification",
            command_mode_popup: "Command Mode (Ctrl+a)",
            command_mode_key_column: "Key",
            multiselect_bulk_title: " Multi-select ",
            multiselect_help_title: "Multi-select (Ctrl+Space)",
            help_section_viewer: "Right Pane",
            help_section_qa: "Quick Actions (Spacebar)",
            help_command: "Command",
            help_action: "Action",
            ublx_switch_popup: "UBLX Switcher",
            ublx_switch_column_path: "UBLX projects",
            ublx_switch_empty: "No indexed projects found (recents empty or no DB under ubli/).",
        }
    }

    const fn viewer_raster() -> UiStringsViewerRaster {
        UiStringsViewerRaster {
            page_label: "Page",
            could_not_load_preview: "Could not load preview",
            could_not_decode_cover: "Could not decode cover",
            could_not_open_image: "Could not open image",
        }
    }

    const fn toasts() -> UiStringsToasts {
        UiStringsToasts {
            config_reloaded: "Config reloaded",
            no_duplicates: "No duplicates found",
            force_full_enhance_background: "Getting metadata for all files.",
            enhanced_with_zahirscan: "Enhanced with ZahirScan",
            enhance_failed_prefix: "Enhance failed: ",
            copied_path_to_clipboard: "Copied path to clipboard",
            copied_zahir_json_to_clipboard: "Copied Zahir JSON to clipboard",
            copy_path_failed_prefix: "Copy path failed: ",
            copy_zahir_json_failed_prefix: "Copy Zahir JSON failed: ",
            removed_from_lens: r#"Removed from lens "{LENS}""#,
            file_renamed: r#"Renamed to "{PATH}""#,
            file_deleted: "Deleted",
            file_ops_failed_prefix: "Failed: ",
            multiselect_none_selected: "No rows selected (Space toggles)",
            bulk_renamed_n: "Renamed {N} file(s)",
            bulk_rename_no_editor: "Bulk rename needs editor_path or $EDITOR",
            bulk_rename_editor_failed: "Could not run editor for bulk rename",
            bulk_rename_no_changes: "No renames (paths unchanged)",
            bulk_removed_n_from_lens: r#"Removed {N} path(s) from lens "{LENS}""#,
            bulk_enhanced_zahir_n: "Enhanced {N} file(s) with ZahirScan",
            duplicate_member_ignored: "Hidden from Duplicates for this session",
            export_zahir_ok: "Exported {N} Zahir JSON file(s)",
            export_zahir_none: "No Zahir JSON to export retake snapshot after adjusting settings/enhance policy",
            export_zahir_failed_prefix: "Zahir export failed: ",
            export_lenses_ok: "Exported {N} lens Markdown file(s)",
            export_lenses_none: "No lenses to export create a lens in the Lenses tab first",
            export_lenses_failed_prefix: "Lens export failed: ",
        }
    }

    const fn file_strings() -> UiStringsFile {
        UiStringsFile {
            rename_prompt: "Rename to: ",
            delete_confirm_title: "Delete ",
        }
    }

    const fn lens() -> UiStringsLens {
        UiStringsLens {
            menu_create_new: "Create New Lens",
            name_prompt: "Lens name: ",
            rename_prompt: "Rename lens: ",
            delete_confirm_title: "Delete lens ",
            delete_yes: "Yes (y)",
            delete_no: "No (n)",
            toast_created_and_added_file: r#"Created lens "{LENS}" and added file"#,
            toast_added_to_lens: r#"Added to lens "{LENS}""#,
            toast_renamed_to: r#"Renamed lens to "{LENS}""#,
            toast_deleted_lens: r#"Deleted lens "{LENS}""#,
        }
    }

    const fn space() -> UiStringsSpaceMenu {
        UiStringsSpaceMenu {
            open: "Open",
            show_in_folder: "Show in folder",
            copy_path: "Copy Path",
            enhance_policy: "Enhance policy",
            enhance_policy_always: "Always — automatic (y)",
            enhance_policy_never: "Per-file — manual (n)",
            enhance_with_zahirscan: "Enhance with ZahirScan",
            copy_zahir_json: "Copy Zahir JSON",
            add_to_lens: "Add to Lens",
            add_to_other_lens: "Add to other lens",
            remove_from_lens: "Delete from current Lens",
            rename: "Rename",
            delete: "Delete",
            ignore_in_duplicates: "Ignore",
        }
    }

    const fn settings_bool() -> UiStringsSettingsBool {
        UiStringsSettingsBool {
            show_hidden_files: "show_hidden_files",
            hash: "hash",
            enable_enhance_all: "enable_enhance_all",
            ask_enhance_on_new_root: "ask_enhance_on_new_root",
            run_snapshot_on_startup: "run_snapshot_on_startup",
            unknown_row: "?",
        }
    }

    const fn settings_pane() -> UiStringsSettingsPane {
        UiStringsSettingsPane {
            global_careful_title: "BE CAREFUL: CHANGING GLOBAL SETTINGS",
            global_careful_detail: "Any change here affects values not set in local",
            typed_column_tables_label: "typed_column_tables: ",
            typed_column_tables_none: " none ",
            typed_column_tables_abbrev: " abbrev ",
            typed_column_tables_full: " full ",
            typed_column_tables_footnote: "none: hide String/Number/… column tables in Metadata; abbrev: cap tables longer than 20 rows to 20 rows; full: show all metadata",
            opacity_format_label: "opacity_format: ",
            rgba_toggle: " rgba ",
            hex8_toggle: " hex8 ",
            edit_enter_save_lock: "Enter to save and lock",
            edit_enter_unlock: "Enter to unlock",
            edit_layout_template: "Edit layout ({})",
            layout_left_pct: "left_pct ",
            layout_middle_pct: "middle_pct ",
            layout_right_pct: "right_pct ",
            edit_opacity_template: "Edit background opacity ({})",
            opacity_value_label: "value ",
            opacity_format_footnote: "OSC 11 encoding for transparent background; may require full restart",
            external_apps_title: "External apps",
            ffmpeg_label: "FFmpeg: ",
            tool_available: "available",
            tool_not_found: "not found",
            pdf_label: "PDF: ",
            pdf_backends_poppler_and_mupdf: "Poppler (pdftoppm) · MuPDF (mutool)",
            pdf_backends_poppler_only: "Poppler (pdftoppm) only",
            pdf_backends_mupdf_only: "MuPDF (mutool) only",
            snapshot_applied_footnote: "* settings applied on next snapshot",
            yn_yes: " Yes ",
            yn_no: " No ",
            right_pane_title: " File ",
            path_global_unavailable: "(global config path unavailable)",
            path_local_missing: "(no local ublx.toml / .ublx.toml)",
        }
    }

    const fn first_run() -> UiStringsFirstRun {
        UiStringsFirstRun {
            welcome_title: "Welcome to UBLX",
            root_choice_footer: "Enter — confirm   Esc / q — quit",
            ublx_here: "New UBLX here: ",
            recent_heading: "Recent UBLX",
            enhance_prompt_title: "Index with full ZahirScan for all files automatically?",
            enhance_prompt_footnote: "Not recommended for very large directories.\nChange anytime in Settings (`enable_enhance_all`).\nTo turn off this prompt: `ask_enhance_on_new_root = false` in Settings (Global).\nDefault is off unless you set `enable_enhance_all = true`.",
            enhance_yes: "Yes (y)",
            enhance_no: "No (n)",
            previous_settings_title: "Previous settings found",
            previous_settings_footnote: "Use saved: keep or restore `ublx.toml` from the last run.\nStart fresh: remove local and cached config, then continue setup.\nGlobal config remains unchanged.",
            previous_settings_use: "Use saved settings (y)",
            previous_settings_fresh: "Start from scratch (n)",
        }
    }

    #[must_use]
    pub const fn new() -> Self {
        Self {
            misc: Self::misc(),
            delta: Self::delta(),
            pane: Self::pane(),
            list: Self::list(),
            main_tabs: Self::main_tabs(),
            search: Self::search(),
            config: Self::config(),
            paths: Self::paths(),
            brand: Self::brand(),
            tables: Self::tables(),
            dialogs: Self::dialogs(),
            viewer_raster: Self::viewer_raster(),
            toasts: Self::toasts(),
            lens: Self::lens(),
            space: Self::space(),
            file: Self::file_strings(),
            first_run: Self::first_run(),
            settings_bool: Self::settings_bool(),
            settings_pane: Self::settings_pane(),
        }
    }

    /// Padded label width so **Dark** / **Light** section lines share the same total width.
    pub const THEME_SELECTOR_SECTION_LABEL_WIDTH: usize = 5;

    /// Theme picker section row: indent, left rule, padded label, spaces, right rule. **Dark** uses a single space before the right rule and one extra trailing `─` (see [`Self::theme_selector_section_row_dark`]).
    #[must_use]
    pub fn theme_selector_section_row(&self, label: &str) -> String {
        if label == "Dark" {
            self.theme_selector_section_row_dark()
        } else {
            format!(
                "   {} {:width$} {}",
                UI_GLYPHS.theme_section_rule,
                label,
                UI_GLYPHS.theme_section_rule,
                width = Self::THEME_SELECTOR_SECTION_LABEL_WIDTH
            )
        }
    }

    /// Dark section: `───` + padded `Dark` + `───` + `─` (no extra space before the right rule; avoids pad + separator doubling).
    #[must_use]
    pub fn theme_selector_section_row_dark(&self) -> String {
        format!(
            "   {} {:width$}{}{}",
            UI_GLYPHS.theme_section_rule,
            "Dark",
            UI_GLYPHS.theme_section_rule,
            '\u{2500}',
            width = Self::THEME_SELECTOR_SECTION_LABEL_WIDTH
        )
    }

    /// Display width of a section row (same for Dark and Light layouts; keep in sync with [`Self::theme_selector_section_row`]).
    #[must_use]
    pub fn theme_selector_section_row_width(&self) -> usize {
        3 + 2 * UI_GLYPHS.theme_section_rule.chars().count()
            + 1
            + Self::THEME_SELECTOR_SECTION_LABEL_WIDTH
            + 1
    }

    /// PDF page footer (`Page 2 / 10` or `Page 2` when total unknown).
    #[must_use]
    pub fn viewer_pdf_page_footer(&self, page: u32, page_count: Option<u32>) -> String {
        let p = page.max(1);
        match page_count {
            Some(n) => format!("{} {p} / {n}", self.viewer_raster.page_label),
            None => format!("{} {p}", self.viewer_raster.page_label),
        }
    }

    #[must_use]
    pub fn viewer_err_load_preview(&self, e: impl std::fmt::Display) -> String {
        format!("{}: {}", self.viewer_raster.could_not_load_preview, e)
    }

    #[must_use]
    pub fn viewer_err_decode_cover(&self, e: impl std::fmt::Display) -> String {
        format!("{}: {}", self.viewer_raster.could_not_decode_cover, e)
    }

    #[must_use]
    pub fn viewer_err_open_image(&self, e: impl std::fmt::Display) -> String {
        format!("{}: {}", self.viewer_raster.could_not_open_image, e)
    }
}

pub const UI_STRINGS: UiStrings = UiStrings::new();
