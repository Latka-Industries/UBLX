//! Browser keyboard map — TUI-shaped subset for list/pane/mode navigation.

use web_sys::KeyboardEvent;

use crate::nav::MainMode;
use crate::panes::RightTab;

/// Actions the web shell handles (PR #1 hotkeys). Viewer-find / Command Mode later.
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
}

/// Map a keydown to a [`WebAction`]. Returns `None` when the event should pass through.
///
/// Caller must skip invoking this while a form field (including catalog search input) is focused.
/// When `help_open`, only help navigation / close / toggle are returned.
#[must_use]
pub(crate) fn action_from_keydown(ev: &KeyboardEvent, help_open: bool) -> Option<WebAction> {
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

    if shift && !ctrl && key == "Tab" {
        return Some(WebAction::CycleRightTab);
    }

    // Preview scroll / PDF page nav (TUI Shift+J/K/B/E + Shift+arrows).
    if shift && !ctrl {
        match (key.as_str(), code.as_str()) {
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
            "Escape" => return None, // reserved; search handles its own Esc
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
        // Plain `s` cycles sort; Shift+S reserved for viewer find (later mini-PR).
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
