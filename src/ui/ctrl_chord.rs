//! **Command Mode** (after **Ctrl+{leader}**, default **Ctrl+A**): the next letter runs the matching shortcut (d, t, s, r, x, l, p),
//! or after a short timeout a centered menu lists them. Viewer search is **Shift+S**, not Command Mode.
//! Jump-by-10 stays **Ctrl+j** / **Ctrl+k** (or arrows), not Command Mode.
//! **Ctrl+Space** toggles middle-pane multi-select when contents are focused; Command Mode uses the configured leader letter.

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::app::RunUblxParams;
use crate::handlers::state_transitions;
use crate::layout::setup::{
    CtrlChordState, MainMode, RightPaneContent, UblxState, ViewData, ViewerChrome,
};
use crate::modules;
use crate::ui::{COMMAND_MODE_DESCRIPTIONS, MainTabFlags, keymap::UblxAction};

/// After leader, show the menu if no second key within this duration.
pub const CHORD_MENU_DELAY: Duration = Duration::from_millis(480);

fn chord_blocked(state: &UblxState) -> bool {
    // Settings edits the leader letter with plain keys — do not start a chord there.
    state.main_mode == MainMode::Settings
        || state.chrome.help_visible
        || state.theme.selector_visible
        || state.qa_menu.visible
        || state.open_menu.visible
        || state.lens_menu.visible
        || state.enhance_policy_menu.visible
        || state.startup_prompt.is_some()
        || state.file_rename_input.is_some()
        || state.file_delete_confirm.visible
        || state.multiselect.bulk_menu_visible
        || state.lens_confirm.delete_visible
        || state.lens_confirm.rename_input.is_some()
}

/// Start chord wait (highlight chrome); caller must ensure key was the configured Ctrl+leader.
pub fn try_begin_chord(state: &mut UblxState) -> bool {
    if chord_blocked(state) || state.search.active {
        return false;
    }
    let cc = &mut state.chrome.ctrl_chord;
    cc.pending = true;
    cc.menu_visible = false;
    cc.started = Some(Instant::now());
    true
}

pub fn end_chord(state: &mut UblxState) {
    state.chrome.ctrl_chord = CtrlChordState::default();
}

pub fn tick_chord_menu_timeout(state: &mut UblxState, now: Instant) {
    let cc = &mut state.chrome.ctrl_chord;
    if !cc.pending || cc.menu_visible {
        return;
    }
    let Some(started) = cc.started else {
        return;
    };
    if now.duration_since(started) >= CHORD_MENU_DELAY {
        cc.menu_visible = true;
    }
}

fn chord_action_for_key(c: char) -> Option<UblxAction> {
    let c = c.to_ascii_lowercase();
    match c {
        'd' => Some(UblxAction::LoadDuplicates),
        't' => Some(UblxAction::ThemeSelector),
        's' => Some(UblxAction::TakeSnapshot),
        'r' => Some(UblxAction::ReloadConfig),
        'x' => Some(UblxAction::ExportZahirJson),
        'l' => Some(UblxAction::ExportLensMarkdown),
        _ => None,
    }
}

fn apply_chord_action(
    state: &mut UblxState,
    view: &ViewData,
    right: &RightPaneContent,
    action: UblxAction,
    tabs: MainTabFlags,
    params: &mut RunUblxParams<'_>,
) -> bool {
    let mode_before = state.main_mode;
    let ctx = state_transitions::UblxActionContext::new(view, right);
    let quit = ctx.apply_action_to_state(state, action, tabs.has_duplicates, tabs.has_lenses);
    if state.main_mode == MainMode::Settings && mode_before != MainMode::Settings {
        modules::settings::on_enter_settings(state, params);
    }
    quit
}

/// If Command Mode is active, handle the key and return `Some(quit)`; `None` means not in Command Mode.
#[must_use]
pub fn handle_chord_key_event(
    state: &mut UblxState,
    e: &KeyEvent,
    view: &ViewData,
    right_content: &RightPaneContent,
    params: &mut RunUblxParams<'_>,
    tabs: MainTabFlags,
    theme_ctx: &modules::theme_selector::ThemeContext,
) -> Option<bool> {
    if !state.chrome.ctrl_chord.is_active() {
        return None;
    }
    if e.kind != KeyEventKind::Press {
        return Some(false);
    }

    match e.code {
        KeyCode::Esc => {
            end_chord(state);
            Some(false)
        }
        KeyCode::Char(c) => {
            if c.eq_ignore_ascii_case(&'p') {
                end_chord(state);
                modules::ublx_switch::open(state, params);
                return Some(false);
            }
            let Some(action) = chord_action_for_key(c) else {
                end_chord(state);
                return Some(false);
            };
            end_chord(state);
            if matches!(action, UblxAction::ThemeSelector) && !state.theme.selector_visible {
                modules::theme_selector::open(state, theme_ctx);
                return Some(false);
            }
            let quit = apply_chord_action(state, view, right_content, action, tabs, params);
            Some(quit)
        }
        _ => Some(false),
    }
}

/// `ViewerChrome` chord highlight for tabs / status.
#[must_use]
pub fn chord_chrome_active(chrome: &ViewerChrome) -> bool {
    chrome.ctrl_chord.is_active()
}

/// Labels for the centered Command Mode menu (letter → description). Descriptions: [`crate::ui::COMMAND_MODE_DESCRIPTIONS`].
pub const CTRL_MENU_ROWS: &[(&str, &str)] = &[
    ("d", COMMAND_MODE_DESCRIPTIONS.duplicates),
    ("t", COMMAND_MODE_DESCRIPTIONS.theme),
    ("s", COMMAND_MODE_DESCRIPTIONS.snapshot),
    ("r", COMMAND_MODE_DESCRIPTIONS.reload),
    ("x", COMMAND_MODE_DESCRIPTIONS.export_zahir),
    ("l", COMMAND_MODE_DESCRIPTIONS.export_lenses),
    ("p", COMMAND_MODE_DESCRIPTIONS.project),
];
