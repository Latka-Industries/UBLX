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
}

/// Map a keydown to a [`WebAction`]. Returns `None` when the event should pass through.
#[must_use]
pub(crate) fn action_from_keydown(ev: &KeyboardEvent, search_active: bool) -> Option<WebAction> {
    if search_active {
        return None;
    }

    let key = ev.key();
    let ctrl = ev.ctrl_key() || ev.meta_key();
    let alt = ev.alt_key();
    let shift = ev.shift_key();

    if alt {
        return None;
    }

    if shift && !ctrl && key == "Tab" {
        return Some(WebAction::CycleRightTab);
    }

    // Mode digits / search / toggle (ignore when Ctrl held).
    if !ctrl {
        match key.as_str() {
            "1" if !shift => return Some(WebAction::MainMode(MainMode::Snapshot)),
            "2" if !shift => return Some(WebAction::MainMode(MainMode::Lenses)),
            "7" if !shift => return Some(WebAction::MainMode(MainMode::Delta)),
            "8" if !shift => return Some(WebAction::MainMode(MainMode::Duplicates)),
            "9" if !shift => return Some(WebAction::MainMode(MainMode::Settings)),
            "/" if !shift => return Some(WebAction::SearchStart),
            "~" => return Some(WebAction::MainModeToggle),
            "Tab" if !shift => return Some(WebAction::FocusCycle),
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
