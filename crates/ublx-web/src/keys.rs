//! Browser keyboard map — TUI-shaped subset for list/pane/mode navigation.

use web_sys::KeyboardEvent;

use crate::nav::MainMode;
use crate::panes::RightTab;

/// Actions the web shell handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WebAction {
    SearchStart,
    MainMode(MainMode),
    MainModeToggle,
    FocusLeft,
    FocusMiddle,
    FocusCycle,
    MoveUp,
    MoveDown,
    MoveUpFast,
    MoveDownFast,
    ListTop,
    ListBottom,
    RightTab(RightTab),
    CycleRightTab,
    HelpToggle,
    HelpClose,
    HelpSectionNext,
    HelpSectionPrev,
    /// Help is open — consume the key without side effects.
    HelpAbsorb,
    /// Cycle middle-pane content sort (`s`) — Snapshot / Dupes / Delta.
    CycleContentSort,
    /// Right-pane preview: scroll down / PDF next page (Shift+J / Shift+↓).
    ScrollPreviewDown,
    /// Right-pane preview: scroll up / PDF previous page (Shift+K / Shift+↑).
    ScrollPreviewUp,
    /// Right-pane preview: top / PDF first page (Shift+B).
    PreviewTop,
    /// Right-pane preview: bottom / PDF last page (Shift+E).
    PreviewBottom,
    /// Open / re-edit Viewer find (Shift+S).
    ViewerFindOpen,
    /// Next find match (`n` while find committed).
    ViewerFindNext,
    /// Previous find match (`N` while find committed).
    ViewerFindPrev,
    /// Clear Viewer find (Esc while committed).
    ViewerFindClear,
    /// Toggle right-pane fullscreen (Shift+F).
    ViewerFullscreenToggle,
    /// Exit right-pane fullscreen (Esc / q while fullscreen).
    ViewerFullscreenExit,
    /// Ctrl+Space — enter/exit multi-select (Snapshot / Lenses, contents).
    MultiselectToggleMode,
    /// Space — toggle cursor row while multi-select is active.
    MultiselectToggleRow,
    /// Esc — exit multi-select.
    MultiselectCancel,
    /// `a` — open bulk menu while multi-select is active.
    MultiselectOpenBulk,
    /// Space — open quick-actions (when multi-select is off).
    SpaceMenuOpen,
    SpaceMenuMoveUp,
    SpaceMenuMoveDown,
    SpaceMenuSubmit,
    SpaceMenuClose,
    /// Letter hotkey while Space / bulk menu is open.
    SpaceMenuHotkey(char),
    /// Menu open — swallow unmatched keys.
    SpaceMenuAbsorb,
    /// Ctrl+leader — start Command Mode chord.
    CommandModeBegin,
    CommandModeClose,
    CommandModeHotkey(char),
    CommandModeMoveUp,
    CommandModeMoveDown,
    CommandModeSubmit,
    CommandModeAbsorb,
}

/// Extra keymap gates for Viewer find (TUI `KeyActionContext` subset).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FindKeyCtx {
    /// Find bar committed (n/N / Esc clear).
    pub committed: bool,
    /// Catalog `/` search is typing — block Shift+S.
    pub catalog_search_active: bool,
    /// False in Settings (TUI).
    pub allow: bool,
}

/// Extra keymap gates for multi-select.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MultiselectKeyCtx {
    pub active: bool,
    /// Snapshot or Lenses.
    pub applies: bool,
    /// Contents (middle) pane focused.
    pub middle_focused: bool,
}

/// Extra keymap gates for Space / bulk menu.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SpaceMenuKeyCtx {
    pub open: bool,
    /// Snapshot / Lenses / Duplicates (not Delta / Settings).
    pub can_open: bool,
}

/// Extra keymap gates for Command Mode (Ctrl+leader).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CommandModeKeyCtx {
    /// Chord pending, menu open, or theme/root picker open.
    pub active: bool,
    /// Theme / root picker (j/k Enter instead of letter actions).
    pub picker: bool,
    /// Help, Space menu, Settings, catalog search — do not begin chord.
    pub blocked: bool,
    pub leader: char,
}

/// Map a keydown to a [`WebAction`]. Returns `None` when the event should pass through.
///
/// Caller must skip invoking this while a form field (including catalog / find inputs) is focused.
/// When `help_open`, only help navigation / close / toggle are returned.
#[must_use]
pub(crate) fn action_from_keydown(
    ev: &KeyboardEvent,
    help_open: bool,
    find: FindKeyCtx,
    ms: MultiselectKeyCtx,
    space: SpaceMenuKeyCtx,
    cmd: CommandModeKeyCtx,
    viewer_fullscreen: bool,
) -> Option<WebAction> {
    let key = ev.key();
    let code = ev.code();
    let ctrl = ev.ctrl_key() || ev.meta_key();
    let alt = ev.alt_key();
    let shift = ev.shift_key();

    if alt {
        return None;
    }

    // `?` is Shift+/ on US layouts — `key` is "?" when shift is held.
    if !ctrl && (key == "?" || (shift && code == "Slash")) {
        return Some(WebAction::HelpToggle);
    }

    if help_open {
        if !ctrl && key == "Escape" {
            return Some(WebAction::HelpClose);
        }
        if !ctrl && !shift && key == "Tab" {
            return Some(WebAction::HelpSectionNext);
        }
        if shift && !ctrl && key == "Tab" {
            return Some(WebAction::HelpSectionPrev);
        }
        if !ctrl && !shift {
            match key.as_str() {
                "ArrowRight" | "l" | "L" => return Some(WebAction::HelpSectionNext),
                "ArrowLeft" | "h" | "H" => return Some(WebAction::HelpSectionPrev),
                _ => {}
            }
        }
        // Swallow other keys while help is open (match TUI overlay).
        return Some(WebAction::HelpAbsorb);
    }

    // Command Mode chord / pickers (before Space menu so Esc closes the right overlay).
    if cmd.active {
        if !ctrl && key == "Escape" {
            return Some(WebAction::CommandModeClose);
        }
        if cmd.picker {
            if !ctrl && !shift && (key == "Enter" || key == " ") {
                return Some(WebAction::CommandModeSubmit);
            }
            if !ctrl && !shift {
                match key.as_str() {
                    "ArrowUp" | "k" | "K" => return Some(WebAction::CommandModeMoveUp),
                    "ArrowDown" | "j" | "J" => return Some(WebAction::CommandModeMoveDown),
                    _ => {}
                }
                match code.as_str() {
                    "ArrowUp" => return Some(WebAction::CommandModeMoveUp),
                    "ArrowDown" => return Some(WebAction::CommandModeMoveDown),
                    _ => {}
                }
            }
            return Some(WebAction::CommandModeAbsorb);
        }
        if !ctrl && !shift && key.len() == 1 {
            let c = key.chars().next()?.to_ascii_lowercase();
            if c.is_ascii_alphabetic() {
                return Some(WebAction::CommandModeHotkey(c));
            }
        }
        return Some(WebAction::CommandModeAbsorb);
    }

    // Ctrl+leader begins Command Mode (not in Settings / help / search / Space menu).
    // Exact letter / `KeyA`-style code only — never `starts_with` (ArrowUp/Down begin with 'a').
    if ctrl && !shift && !cmd.blocked && ctrl_command_mode_leader_hit(&key, &code, cmd.leader) {
        return Some(WebAction::CommandModeBegin);
    }

    if space.open {
        if !ctrl && key == "Escape" {
            return Some(WebAction::SpaceMenuClose);
        }
        if !ctrl && !shift && (key == "Enter" || key == " ") {
            return Some(WebAction::SpaceMenuSubmit);
        }
        // Letter hotkeys win over j/k move (TUI: `j` = Copy Zahir JSON when that row exists).
        if !ctrl && !shift && key.len() == 1 {
            let c = key.chars().next()?.to_ascii_lowercase();
            if c.is_ascii_alphabetic() || c.is_ascii_digit() {
                return Some(WebAction::SpaceMenuHotkey(c));
            }
        }
        if !ctrl && !shift {
            match key.as_str() {
                "ArrowUp" => return Some(WebAction::SpaceMenuMoveUp),
                "ArrowDown" => return Some(WebAction::SpaceMenuMoveDown),
                _ => {}
            }
        }
        return Some(WebAction::SpaceMenuAbsorb);
    }

    if shift && !ctrl && key == "Tab" {
        return Some(WebAction::CycleRightTab);
    }

    // Viewer find: Shift+S open; n/N next/prev when committed; Esc clears committed.
    if shift && !ctrl && find.allow && !find.catalog_search_active {
        match key.as_str() {
            "S" | "s" => return Some(WebAction::ViewerFindOpen),
            _ => {}
        }
    }
    if find.committed && !ctrl {
        if key == "Escape" {
            return Some(WebAction::ViewerFindClear);
        }
        if !shift && (key == "n" || key == "N") {
            return Some(WebAction::ViewerFindNext);
        }
        if shift && (key == "N" || key == "n" || code == "KeyN") {
            return Some(WebAction::ViewerFindPrev);
        }
    }

    // Multi-select: Ctrl+Space toggle; Space row toggle; Esc exit (after find Esc).
    if ctrl && !shift && (key == " " || code == "Space") {
        if ms.applies && ms.middle_focused {
            return Some(WebAction::MultiselectToggleMode);
        }
        return None;
    }
    if ms.active && !ctrl && !shift && (key == " " || code == "Space") {
        return Some(WebAction::MultiselectToggleRow);
    }
    if ms.active && !ctrl && key == "Escape" {
        return Some(WebAction::MultiselectCancel);
    }
    if ms.active && !ctrl && !shift && (key == "a" || key == "A") {
        return Some(WebAction::MultiselectOpenBulk);
    }

    // TUI apply_quit: Esc / q leave fullscreen before anything else app-level.
    if viewer_fullscreen && !ctrl {
        if key == "Escape" {
            return Some(WebAction::ViewerFullscreenExit);
        }
        if !shift && (key == "q" || key == "Q") {
            return Some(WebAction::ViewerFullscreenExit);
        }
    }

    // Space opens quick-actions when multi-select is off.
    if !ms.active && !ctrl && !shift && (key == " " || code == "Space") && space.can_open {
        return Some(WebAction::SpaceMenuOpen);
    }

    // Preview scroll / PDF page nav (TUI Shift+J/K/B/E + Shift+arrows).
    // After Shift+S handling so find open wins over nothing on S.
    // Shift+F toggles viewer fullscreen (TUI ViewerFullscreenToggle).
    if shift && !ctrl {
        match (key.as_str(), code.as_str()) {
            ("F" | "f", _) | (_, "KeyF") => return Some(WebAction::ViewerFullscreenToggle),
            ("J" | "j", _) | (_, "ArrowDown") => return Some(WebAction::ScrollPreviewDown),
            ("K" | "k", _) | (_, "ArrowUp") => return Some(WebAction::ScrollPreviewUp),
            ("B" | "b", _) => return Some(WebAction::PreviewTop),
            ("E" | "e", _) => return Some(WebAction::PreviewBottom),
            _ => {}
        }
    }

    // Mode digits via `code` (layout-stable) + numpad.
    if !ctrl && !shift {
        let digit_mode = match code.as_str() {
            "Digit1" | "Numpad1" => Some(MainMode::Snapshot),
            "Digit2" | "Numpad2" => Some(MainMode::Lenses),
            "Digit7" | "Numpad7" => Some(MainMode::Delta),
            "Digit8" | "Numpad8" => Some(MainMode::Duplicates),
            "Digit9" | "Numpad9" => Some(MainMode::Settings),
            _ => None,
        };
        if let Some(m) = digit_mode {
            return Some(WebAction::MainMode(m));
        }
    }

    // Search / toggle / Tab (ignore when Ctrl held).
    if !ctrl {
        match key.as_str() {
            "/" if !shift => return Some(WebAction::SearchStart),
            "~" => return Some(WebAction::MainModeToggle),
            "Tab" if !shift => return Some(WebAction::FocusCycle),
            "Escape" => return None, // catalog search input handles its own Esc
            _ => {}
        }
    }

    // Letters: use ASCII lower so Shift does not dead-end chords like `G` / `V`.
    let lower = key.to_ascii_lowercase();
    match lower.as_str() {
        "v" if !ctrl => return Some(WebAction::RightTab(RightTab::Viewer)),
        "t" if !ctrl => return Some(WebAction::RightTab(RightTab::Templates)),
        "m" if !ctrl => return Some(WebAction::RightTab(RightTab::Metadata)),
        "w" if !ctrl => return Some(WebAction::RightTab(RightTab::Writing)),
        "h" if !ctrl && !shift => return Some(WebAction::FocusLeft),
        "l" if !ctrl && !shift => return Some(WebAction::FocusMiddle),
        "k" if !shift => {
            return Some(if ctrl {
                WebAction::MoveUpFast
            } else {
                WebAction::MoveUp
            });
        }
        "j" if !shift => {
            return Some(if ctrl {
                WebAction::MoveDownFast
            } else {
                WebAction::MoveDown
            });
        }
        "g" if !ctrl && !shift && key == "g" => return Some(WebAction::ListTop),
        "g" if !ctrl && key == "G" => return Some(WebAction::ListBottom),
        "s" if !ctrl && !shift && key == "s" => return Some(WebAction::CycleContentSort),
        _ => {}
    }

    match key.as_str() {
        "ArrowLeft" if !ctrl && !shift => Some(WebAction::FocusLeft),
        "ArrowRight" if !ctrl && !shift => Some(WebAction::FocusMiddle),
        "ArrowUp" if !shift => Some(if ctrl {
            WebAction::MoveUpFast
        } else {
            WebAction::MoveUp
        }),
        "ArrowDown" if !shift => Some(if ctrl {
            WebAction::MoveDownFast
        } else {
            WebAction::MoveDown
        }),
        _ => None,
    }
}

/// True when `key` / `code` is exactly the Command Mode leader (e.g. `a` / `KeyA`).
///
/// Must not use prefix matching: browser `key` for arrows is `ArrowUp` / `ArrowDown`, both of
/// which start with `a` and would falsely open Command Mode on Ctrl+↑/↓.
#[must_use]
fn ctrl_command_mode_leader_hit(key: &str, code: &str, leader: char) -> bool {
    let leader = leader.to_ascii_lowercase();
    if !leader.is_ascii_alphabetic() {
        return false;
    }
    let leader_s = leader.to_string();
    (key.len() == 1 && key.eq_ignore_ascii_case(&leader_s))
        || code.eq_ignore_ascii_case(&format!("Key{}", leader.to_ascii_uppercase()))
}

pub(crate) fn typing_in_form_field() -> bool {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    let Some(el) = doc.active_element() else {
        return false;
    };
    let tag = el.tag_name().to_ascii_lowercase();
    matches!(tag.as_str(), "input" | "textarea" | "select")
        || el.get_attribute("contenteditable").is_some()
}

#[cfg(test)]
mod tests {
    use super::ctrl_command_mode_leader_hit;

    #[test]
    fn leader_a_matches_letter_and_code() {
        assert!(ctrl_command_mode_leader_hit("a", "", 'a'));
        assert!(ctrl_command_mode_leader_hit("A", "KeyB", 'a')); // key letter wins
        assert!(ctrl_command_mode_leader_hit("", "KeyA", 'a'));
        assert!(!ctrl_command_mode_leader_hit("b", "KeyB", 'a'));
    }

    #[test]
    fn leader_a_does_not_match_arrow_keys() {
        // Regression: `starts_with('a')` matched ArrowUp / ArrowDown.
        assert!(!ctrl_command_mode_leader_hit("ArrowUp", "ArrowUp", 'a'));
        assert!(!ctrl_command_mode_leader_hit("ArrowDown", "ArrowDown", 'a'));
        assert!(!ctrl_command_mode_leader_hit("ArrowLeft", "ArrowLeft", 'a'));
        assert!(!ctrl_command_mode_leader_hit(
            "ArrowRight",
            "ArrowRight",
            'a'
        ));
    }
}
