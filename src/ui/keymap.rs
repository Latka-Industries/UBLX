//! Key bindings for the vanilla TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::consts::UblxTabNumber;

/// Actions for the 3-panel TUI (categories, contents, preview).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UblxAction {
    /// Exit the TUI (`q` when not searching; Esc when search is inactive).
    Quit,
    /// Open the full keybinding help overlay (`?`).
    Help,
    /// Switch to Snapshot main tab.
    MainModeSnapshot,
    /// Switch to Delta main tab.
    MainModeDelta,
    /// Switch to Duplicates main tab (only when duplicates exist).
    MainModeDuplicates,
    /// Switch to Settings main tab (global / local `ublx.toml`).
    MainModeSettings,
    /// Switch to Lenses main tab (only when any lenses exist).
    MainModeLenses,
    /// Alternate main tabs: Snapshot → Lenses (if any) → Delta → Duplicates (if any) → Settings (`~`)
    MainModeToggle,
    /// Run duplicate detection in background and show Duplicates tab (Command Mode: Ctrl+A, then d).
    LoadDuplicates,
    /// Open the catalog path filter (`/`).
    SearchStart,
    /// Type into the catalog search bar (while active).
    SearchChar(char),
    /// Delete in the catalog search bar.
    SearchBackspace,
    /// Commit catalog search (Enter).
    SearchSubmit,
    /// Esc when search is active (clear search); when inactive, use Quit.
    SearchClear,
    /// In-pane literal search (right pane). Shift+S opens; Enter commits; Esc clears; n / N next/prev.
    ViewerFindOpen,
    ViewerFindChar(char),
    ViewerFindBackspace,
    ViewerFindSubmit,
    ViewerFindClear,
    ViewerFindNext,
    ViewerFindPrev,
    /// Cycle right pane tab (Shift+Tab).
    CycleRightPane,
    /// Focus the Viewer tab in the right pane (`v`).
    RightPaneViewer,
    /// Toggle right-pane fullscreen (current tab).
    ViewerFullscreenToggle,
    /// Focus the Templates tab (`t`).
    RightPaneTemplates,
    /// Focus the Metadata tab (`m`).
    RightPaneMetadata,
    /// Focus the Writing tab (`w`).
    RightPaneWriting,
    /// Scroll the right-pane preview up (Shift+↑ / Shift+K, or `K` when mapped as navigation).
    ScrollPreviewUp,
    /// Scroll the right-pane preview down (Shift+↓ / Shift+J, or `J` when mapped as navigation).
    ScrollPreviewDown,
    /// gg: go to top of list (categories or contents).
    ListTop,
    /// G: go to bottom of list.
    ListBottom,
    /// Shift+b: scroll preview to top.
    PreviewTop,
    /// Shift+e: scroll preview to bottom.
    PreviewBottom,
    /// Move selection up in the focused list (`k` / Up).
    MoveUp,
    /// Move selection down (`j` / Down).
    MoveDown,
    /// Page-style move up (Ctrl+k / Ctrl+Up).
    MoveUpFast,
    /// Page-style move down (Ctrl+j / Ctrl+Down).
    MoveDownFast,
    /// Focus the categories (left) pane (`h` / Left).
    FocusCategories,
    /// Focus the contents (middle) pane (`l` / Right).
    FocusContents,
    /// Cycle focus between left and middle panes (Tab).
    Tab,
    /// Run take-snapshot pipeline in background; completion shows in log bumper (Command Mode: Ctrl+A, then s).
    TakeSnapshot,
    /// Cycle middle-pane content sort mode (Name → Size → Mod).
    CycleContentSort,
    /// Theme selector popup (Command Mode: Ctrl+A, then t). Writes theme to **local** project `ublx.toml` / `.ublx.toml`.
    ThemeSelector,
    /// Open the active Settings file in $EDITOR / `editor_path` (plain `e`).
    OpenConfigInEditor,
    /// Reload hot-reloadable config (theme, layout, `enable_enhance_all`, hash, `show_hidden`, etc.) from disk (Command Mode: Ctrl+A, then r).
    ReloadConfig,
    /// Export Zahir JSON files to `ublx-export/` (same as `ublx -x`; Command Mode: Ctrl+A, then x).
    ExportZahirJson,
    /// Export lens Markdown files to `ublx-lenses/` (Command Mode: Ctrl+A, then l).
    ExportLensMarkdown,
    /// Middle-pane multi-select: toggle current row (Space while multi-select is on).
    MultiselectToggleRow,
    /// Open bulk action menu (**a** with multi-select on, contents focused, modals closed).
    MultiselectOpenBulkMenu,
    /// Lenses tab, contents, single-select: **a** opens Add to other lens (same picker as bulk **a**; excludes active lens).
    AddToOtherLens,
    /// Bulk menu row by letter (a / r / d; optional z for Zahir).
    BulkMenuHotkeySelect(usize),
    /// Esc while multi-select is active (no search): leave multi-select without quitting.
    MultiselectCancel,
    /// Spacebar context menu
    SpaceMenu,
    /// Pick a context-menu row by letter while the quick actions menu (spacebar) is open (indices match current items).
    SpaceMenuHotkeySelect(usize),
    /// First option on a Yes/No (or two-option) confirm overlay (`y`).
    ConfirmYes,
    /// Second option (`n`).
    ConfirmNo,
    /// Key had no mapping; may carry `last_key_for_double` for a pending `g` → `gg`.
    Noop,
}

/// Result of key mapping: action to run and optional "last key" for double-key (e.g. gg).
pub struct KeyActionResult {
    pub action: UblxAction,
    /// Set when key is the first of a possible double (e.g. first `g` for gg); clear on any other key.
    pub last_key_for_double: Option<char>,
}

/// Search bar state for key resolution (`/` open, typed query, Esc behavior).
#[derive(Clone, Copy, Debug)]
pub struct KeySearchState {
    pub active: bool,
    pub has_filter: bool,
}

/// Whether optional main tabs (Duplicates, Lenses) exist for digit keys and toggle.
#[derive(Clone, Copy, Debug)]
pub struct KeyOptionalTabs {
    pub duplicates: bool,
    pub lenses: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewerFinderBools {
    pub typing: bool,
    pub committed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct MultiselectBools {
    pub active: bool,
    pub bulk_menu_visible: bool,
    pub block_bulk_activation: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct AllowBools {
    pub viewer_find: bool,
    pub lens_add_to_other_hotkey: bool,
}

/// UI snapshot needed to resolve keys: search mode, filter, gg tracking, which optional tabs exist.
#[derive(Clone, Copy, Debug)]
pub struct KeyActionContext {
    pub search: KeySearchState,
    pub viewer_find: ViewerFinderBools,
    pub allow: AllowBools,
    pub last_key_for_double: Option<char>,
    pub tabs: KeyOptionalTabs,
    pub tab_keys: UblxTabNumber,
    /// Middle pane multi-select: Space toggles row (not quick actions menu (spacebar)); **a** opens bulk menu.
    pub multiselect: MultiselectBools,
    pub panel_focus_contents: bool,
    /// Lens picker list open (Esc closes it).
    pub lens_menu_list_open: bool,
}

#[must_use]
fn main_mode_action_for_digit(
    c: char,
    keys: UblxTabNumber,
    tabs: KeyOptionalTabs,
) -> Option<UblxAction> {
    let ch = |n: u8| char::from_digit(u32::from(n), 10);
    if ch(keys.snapshot) == Some(c) {
        return Some(UblxAction::MainModeSnapshot);
    }
    if ch(keys.delta) == Some(c) {
        return Some(UblxAction::MainModeDelta);
    }
    if ch(keys.settings) == Some(c) {
        return Some(UblxAction::MainModeSettings);
    }
    if tabs.lenses && ch(keys.lenses) == Some(c) {
        return Some(UblxAction::MainModeLenses);
    }
    if tabs.duplicates && ch(keys.duplicates) == Some(c) {
        return Some(UblxAction::MainModeDuplicates);
    }
    None
}

#[must_use]
fn viewer_find_typing_mapping(event: KeyEvent) -> KeyActionResult {
    let (action, last_key) = match event.code {
        KeyCode::Esc => (UblxAction::ViewerFindClear, None),
        KeyCode::Enter => (UblxAction::ViewerFindSubmit, None),
        KeyCode::Backspace => (UblxAction::ViewerFindBackspace, None),
        KeyCode::Char(c) => (UblxAction::ViewerFindChar(c), None),
        _ => (UblxAction::Noop, None),
    };
    KeyActionResult {
        action,
        last_key_for_double: last_key,
    }
}

/// Esc: viewer find / search / bulk or lens menu / multi-select cancel; otherwise caller maps Esc to Quit.
#[must_use]
fn try_esc_special(ctx: &KeyActionContext) -> Option<(UblxAction, Option<char>)> {
    if ctx.viewer_find.committed {
        return Some((UblxAction::ViewerFindClear, None));
    }
    if ctx.search.active || ctx.search.has_filter {
        return Some((UblxAction::SearchClear, None));
    }
    if ctx.multiselect.bulk_menu_visible || ctx.lens_menu_list_open {
        return Some((UblxAction::SearchClear, None));
    }
    if ctx.multiselect.active && !ctx.search.active {
        return Some((UblxAction::MultiselectCancel, None));
    }
    None
}

/// Shift/Ctrl-specific keys: viewer find, preview scroll, fast list motion, main-tab toggle, gg.
#[must_use]
fn try_char_action_modifiers(
    code: KeyCode,
    shift: bool,
    ctrl: bool,
    ctx: &KeyActionContext,
) -> Option<(UblxAction, Option<char>)> {
    match code {
        KeyCode::Char('s' | 'S') if shift && ctx.allow.viewer_find && !ctx.search.active => {
            Some((UblxAction::ViewerFindOpen, None))
        }
        KeyCode::Char('n')
            if !shift
                && !ctrl
                && ctx.viewer_find.committed
                && !ctx.viewer_find.typing
                && !ctx.search.active =>
        {
            Some((UblxAction::ViewerFindNext, None))
        }
        KeyCode::Char('N')
            if !ctrl
                && ctx.viewer_find.committed
                && !ctx.viewer_find.typing
                && !ctx.search.active =>
        {
            Some((UblxAction::ViewerFindPrev, None))
        }
        KeyCode::Char('f' | 'F') if shift => Some((UblxAction::ViewerFullscreenToggle, None)),
        KeyCode::Char('J') | KeyCode::Down if shift => Some((UblxAction::ScrollPreviewDown, None)),
        KeyCode::Char('K') | KeyCode::Up if shift => Some((UblxAction::ScrollPreviewUp, None)),
        KeyCode::Char('B') if shift => Some((UblxAction::PreviewTop, None)),
        KeyCode::Char('E') if shift => Some((UblxAction::PreviewBottom, None)),
        KeyCode::Char('j' | 'J') | KeyCode::Down if ctrl => Some((UblxAction::MoveDownFast, None)),
        KeyCode::Char('k' | 'K') | KeyCode::Up if ctrl => Some((UblxAction::MoveUpFast, None)),
        KeyCode::Char('~') => Some((UblxAction::MainModeToggle, None)),
        KeyCode::Char('\u{60}') if shift && !ctx.search.active => {
            Some((UblxAction::MainModeToggle, None))
        }
        KeyCode::Char('G') if shift => Some((UblxAction::ListBottom, None)),
        KeyCode::Char('g') if !shift && !ctrl => {
            if ctx.last_key_for_double == Some('g') {
                Some((UblxAction::ListTop, None))
            } else {
                Some((UblxAction::Noop, Some('g')))
            }
        }
        _ => None,
    }
}

#[must_use]
fn key_action_multiselect_lens_or_digit(c: char, ctx: &KeyActionContext) -> Option<UblxAction> {
    if ctx.multiselect.active
        && ctx.panel_focus_contents
        && !ctx.search.active
        && !ctx.multiselect.bulk_menu_visible
        && !ctx.multiselect.block_bulk_activation
        && c == 'a'
    {
        return Some(UblxAction::MultiselectOpenBulkMenu);
    }
    if ctx.allow.lens_add_to_other_hotkey && c == 'a' {
        return Some(UblxAction::AddToOtherLens);
    }
    if ctx.multiselect.active
        && ctx.panel_focus_contents
        && !ctx.search.active
        && !ctx.multiselect.bulk_menu_visible
        && !ctx.multiselect.block_bulk_activation
        && c == ' '
    {
        return Some(UblxAction::MultiselectToggleRow);
    }
    main_mode_action_for_digit(c, ctx.tab_keys, ctx.tabs)
}

#[must_use]
fn key_action_navigation_letter(c: char) -> UblxAction {
    match c {
        ' ' => UblxAction::SpaceMenu,
        'e' => UblxAction::OpenConfigInEditor,
        'v' => UblxAction::RightPaneViewer,
        't' => UblxAction::RightPaneTemplates,
        'm' => UblxAction::RightPaneMetadata,
        'w' => UblxAction::RightPaneWriting,
        's' => UblxAction::CycleContentSort,
        'j' => UblxAction::MoveDown,
        'k' => UblxAction::MoveUp,
        'h' => UblxAction::FocusCategories,
        'l' => UblxAction::FocusContents,
        'J' => UblxAction::ScrollPreviewDown,
        'K' => UblxAction::ScrollPreviewUp,
        _ => UblxAction::Noop,
    }
}

/// Plain character (no Ctrl): multi-select / lens single-a / tab digits / hjkl navigation.
#[must_use]
fn key_action_plain_char(c: char, ctx: &KeyActionContext) -> (UblxAction, Option<char>) {
    if let Some(a) = key_action_multiselect_lens_or_digit(c, ctx) {
        return (a, None);
    }
    (key_action_navigation_letter(c), None)
}

#[must_use]
fn key_action_default(event: KeyEvent, ctx: &KeyActionContext) -> KeyActionResult {
    let shift = event.modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    let (action, last_key) = match event.code {
        KeyCode::Esc => try_esc_special(ctx).unwrap_or((UblxAction::Quit, None)),
        KeyCode::Char('q') => (UblxAction::Quit, None),
        KeyCode::Char('?') => (UblxAction::Help, None),
        KeyCode::Char('/') if !ctx.search.active => (UblxAction::SearchStart, None),
        KeyCode::Char(c) if ctx.search.active => (UblxAction::SearchChar(c), None),
        code => {
            if let Some(t) = try_char_action_modifiers(code, shift, ctrl, ctx) {
                t
            } else {
                match code {
                    KeyCode::Char(c) if shift => (UblxAction::SearchChar(c), None),
                    KeyCode::Char(c) if !ctrl => key_action_plain_char(c, ctx),
                    KeyCode::Enter => (UblxAction::SearchSubmit, None),
                    KeyCode::Backspace => (UblxAction::SearchBackspace, None),
                    KeyCode::Up => (UblxAction::MoveUp, None),
                    KeyCode::Down => (UblxAction::MoveDown, None),
                    KeyCode::Left => (UblxAction::FocusCategories, None),
                    KeyCode::Right => (UblxAction::FocusContents, None),
                    KeyCode::Tab => (UblxAction::Tab, None),
                    KeyCode::BackTab => (UblxAction::CycleRightPane, None),
                    _ => (UblxAction::Noop, None),
                }
            }
        }
    };
    KeyActionResult {
        action,
        last_key_for_double: last_key,
    }
}

/// Map a key event to a vanilla TUI action. Call only when `event.kind == KeyEventKind::Press`.
/// Esc yields `SearchClear` when the search bar is open or when a filter is active (so Esc clears
/// search instead of quitting). Only when not searching at all does Esc mean Quit.
/// Use `last_key_for_double` in `ctx` to detect gg (two g's) for `ListTop`.
/// Use `tabs.duplicates` / `tabs.lenses` so optional tab digits and `MainModeToggle` apply only when the tab exists.
#[must_use]
pub fn key_action_setup(event: KeyEvent, ctx: &KeyActionContext) -> KeyActionResult {
    if event.kind != KeyEventKind::Press {
        return KeyActionResult {
            action: UblxAction::Noop,
            last_key_for_double: None,
        };
    }
    if ctx.viewer_find.typing {
        let r = viewer_find_typing_mapping(event);
        if !matches!(r.action, UblxAction::Noop) {
            return r;
        }
    }
    key_action_default(event, ctx)
}

/// Returns true if the action was handled by the search bar (main loop should skip navigation).
#[must_use]
pub fn search_consumes(action: UblxAction) -> bool {
    matches!(
        action,
        UblxAction::SearchClear
            | UblxAction::SearchSubmit
            | UblxAction::SearchBackspace
            | UblxAction::SearchChar(_)
    )
}

/// Returns true if the action was handled by the in-pane find bar (skip navigation / other handlers).
#[must_use]
pub fn viewer_find_consumes(action: UblxAction) -> bool {
    matches!(
        action,
        UblxAction::ViewerFindClear
            | UblxAction::ViewerFindSubmit
            | UblxAction::ViewerFindBackspace
            | UblxAction::ViewerFindChar(_)
    )
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    use super::{
        AllowBools, KeyActionContext, KeyOptionalTabs, KeySearchState, MultiselectBools,
        UblxAction, ViewerFinderBools, key_action_setup,
    };
    use crate::ui::consts::UblxTabNumber;

    fn ctx() -> KeyActionContext {
        KeyActionContext {
            search: KeySearchState {
                active: false,
                has_filter: false,
            },
            viewer_find: ViewerFinderBools {
                typing: false,
                committed: false,
            },
            allow: AllowBools {
                viewer_find: true,
                lens_add_to_other_hotkey: false,
            },
            last_key_for_double: None,
            tabs: KeyOptionalTabs {
                lenses: false,
                duplicates: false,
            },
            tab_keys: UblxTabNumber::DEFAULT,
            multiselect: MultiselectBools {
                active: false,
                bulk_menu_visible: false,
                block_bulk_activation: false,
            },
            panel_focus_contents: true,
            lens_menu_list_open: false,
        }
    }

    fn press(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: mods,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        }
    }

    #[test]
    fn ctrl_arrows_jump_like_ctrl_jk() {
        let c = ctx();
        assert_eq!(
            key_action_setup(press(KeyCode::Up, KeyModifiers::CONTROL), &c).action,
            UblxAction::MoveUpFast
        );
        assert_eq!(
            key_action_setup(press(KeyCode::Down, KeyModifiers::CONTROL), &c).action,
            UblxAction::MoveDownFast
        );
        assert_eq!(
            key_action_setup(press(KeyCode::Char('k'), KeyModifiers::CONTROL), &c).action,
            UblxAction::MoveUpFast
        );
        assert_eq!(
            key_action_setup(press(KeyCode::Char('j'), KeyModifiers::CONTROL), &c).action,
            UblxAction::MoveDownFast
        );
    }
}
