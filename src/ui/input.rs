use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use std::io;

use crate::app::RunUblxParams;
use crate::config::{LayoutOverlay, UblxOpts};
use crate::handlers::{leave_terminal_for_editor, state_transitions};
use crate::layout::setup::{
    MainMode, PanelFocus, RightPaneContent, StartupPromptPhase, TuiRow, UblxState, ViewData,
};
use crate::modules;
use crate::render::overlays;
use crate::ui::{
    MainTabFlags,
    consts::{MAIN_TAB_KEYS, UI_CONSTANTS, UI_STRINGS},
    ctrl_chord, file_ops, keymap, menus, mouse, multiselect,
};

/// Everything [`handle_ublx_input`] needs to resolve keys and route to modals: current view, optional
/// row slice, right-pane payload, theme popup state, full terminal area, layout percentages, and which main tabs exist.
#[derive(Clone)]
pub struct InputContext<'a> {
    /// Filtered categories + middle list for this tick.
    pub view: &'a ViewData,
    /// Snapshot rows for resolving paths in multi-select (`None` in modes without a shared row slice).
    pub all_rows: Option<&'a [TuiRow]>,
    /// Viewer / templates / metadata / writing content for the selected item.
    pub right_content: &'a RightPaneContent,
    /// Theme selector popup: open/closed state and list position.
    pub theme_ctx: modules::theme_selector::ThemeContext,
    /// Full terminal rectangle (for overlay placement and mouse hit tests).
    pub frame_area: Rect,
    /// Hot-reloaded left/middle/right width percentages.
    pub layout: &'a LayoutOverlay,
    /// Whether Duplicates / Lenses tabs are shown and duplicate grouping mode.
    pub tabs: MainTabFlags,
}

/// View + middle rows + right pane (shared by [`handle_ublx_keyboard`]).
struct ViewPaneRefs<'a> {
    view: &'a ViewData,
    all_rows: Option<&'a [TuiRow]>,
    right: &'a RightPaneContent,
}

/// Key event and resolved action passed into modal handlers
#[derive(Clone, Copy)]
struct ModalInput {
    e: KeyEvent,
    action: keymap::UblxAction,
}

/// Run modal handlers in order; returns true if any handler consumed the event (caller should then return Ok(false)).
/// Keeps the main input function short and makes it easy to add new modals in one place.
///
/// Rust note: a single dispatch function like this is the usual pattern when each handler has different arguments.
/// A true "(guard, handler)" table would require either (1) a common context struct passed to every handler, or
/// (2) `Box<dyn FnOnce() -> bool>` and passing captured state into a runner—often not worth it until you have
/// many more handlers or need to register them dynamically.
fn dispatch_modal_handlers(
    state_mut: &mut UblxState,
    view_ref: &ViewData,
    right_content_ref: &RightPaneContent,
    theme_ctx_ref: &modules::theme_selector::ThemeContext,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    input: ModalInput,
) -> bool {
    let ModalInput { e, action } = input;
    // Table-style: each line is (guard / handler returns true) → we handled the event.
    if modules::first_run::handle_startup_prompt(state_mut, params_mut, ublx_opts_mut, action) {
        return true;
    }
    if file_ops::handle_file_delete_confirm(state_mut, params_mut, action) {
        return true;
    }
    if file_ops::handle_file_rename_input(state_mut, params_mut, e) {
        return true;
    }
    if menus::handle_lens_name_input(state_mut, params_mut, e) {
        return true;
    }
    if menus::handle_lens_rename_input(state_mut, params_mut, e) {
        return true;
    }
    if menus::handle_lens_delete_confirm(state_mut, params_mut, action) {
        return true;
    }
    if state_mut.chrome.ublx_switch.visible {
        modules::ublx_switch::handle_key(state_mut, params_mut, action);
        return true;
    }
    if modules::enhancer::handle_enhance_policy_menu(state_mut, params_mut, ublx_opts_mut, action) {
        return true;
    }
    if menus::handle_qa_menu(state_mut, view_ref, params_mut, ublx_opts_mut, action) {
        return true;
    }
    if menus::handle_lens_menu(state_mut, params_mut, action) {
        return true;
    }
    if state_mut.theme.selector_visible {
        modules::theme_selector::handle_key(
            state_mut,
            params_mut,
            ublx_opts_mut,
            theme_ctx_ref,
            action,
        );
        return true;
    }
    if state_mut.chrome.help_visible {
        let n = overlays::help_tab_count(state_mut.main_mode);
        match action {
            keymap::UblxAction::Tab if n > 0 => {
                let t = state_mut.chrome.help_tab as usize;
                state_mut.chrome.help_tab = ((t + 1) % n) as u8;
                return true;
            }
            keymap::UblxAction::CycleRightPane if n > 0 => {
                let t = state_mut.chrome.help_tab as usize;
                state_mut.chrome.help_tab = ((t + n - 1) % n) as u8;
                return true;
            }
            _ => {
                state_mut.chrome.help_visible = false;
                return true;
            }
        }
    }
    if menus::handle_open_menu(state_mut, params_mut, ublx_opts_mut, action) {
        return true;
    }
    if menus::try_open_qa_menu(state_mut, view_ref, right_content_ref, action) {
        return true;
    }
    false
}

fn try_open_settings_editor_from_menu(
    state_mut: &mut UblxState,
    action: keymap::UblxAction,
    ublx_opts_ref: &UblxOpts,
) -> bool {
    if state_mut.main_mode != MainMode::Settings
        || !matches!(action, keymap::UblxAction::OpenConfigInEditor)
    {
        return false;
    }
    if let Some(ref path) = state_mut.settings.editing_path
        && let Some(ed) = modules::opener::editor_for_open(ublx_opts_ref.editor_path.as_deref())
    {
        let _ = leave_terminal_for_editor();
        let _ = modules::opener::open_in_editor(&ed, path);
        state_mut.session.tick.refresh_terminal_after_editor = true;
    }
    true
}

/// After modals: Settings digit buffers, theme picker, Settings keys, reload, search clear.
fn handle_post_modal_chrome_keys(
    state_mut: &mut UblxState,
    theme_ctx_ref: &modules::theme_selector::ThemeContext,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    e: KeyEvent,
    action: keymap::UblxAction,
) -> bool {
    if state_mut.main_mode == MainMode::Settings
        && (modules::settings::handle_layout_text_key(state_mut, e)
            || modules::settings::handle_opacity_text_key(state_mut, e)
            || modules::settings::handle_command_mode_leader_key(
                state_mut,
                params_mut,
                ublx_opts_mut,
                e,
            ))
    {
        return true;
    }
    if matches!(action, keymap::UblxAction::ThemeSelector) && !state_mut.theme.selector_visible {
        modules::theme_selector::open(state_mut, theme_ctx_ref);
        return true;
    }
    if state_mut.main_mode == MainMode::Settings
        && modules::settings::handle_key(state_mut, params_mut, ublx_opts_mut, action)
    {
        return true;
    }
    if matches!(action, keymap::UblxAction::ReloadConfig) {
        modules::settings::apply_config_reload(
            params_mut,
            ublx_opts_mut,
            state_mut,
            Some(UI_STRINGS.toasts.config_reloaded),
        );
        return true;
    }
    if matches!(action, keymap::UblxAction::ViewerFindClear) {
        modules::viewer_search::clear(state_mut);
        return true;
    }
    if matches!(action, keymap::UblxAction::SearchClear) {
        state_mut.search.query.clear();
        state_mut.search.active = false;
        return true;
    }
    false
}

fn handle_viewer_find_typing(state_mut: &mut UblxState, action: keymap::UblxAction) -> bool {
    if !state_mut.viewer_find.active {
        return false;
    }
    match action {
        keymap::UblxAction::ViewerFindSubmit => {
            state_mut.viewer_find.active = false;
            state_mut.viewer_find.committed = true;
        }
        keymap::UblxAction::ViewerFindBackspace => {
            state_mut.viewer_find.query.pop();
            state_mut.viewer_find.last_sync_token = None;
        }
        keymap::UblxAction::ViewerFindChar(c) => {
            state_mut.viewer_find.query.push(c);
            state_mut.viewer_find.last_sync_token = None;
        }
        _ => return false,
    }
    keymap::viewer_find_consumes(action)
}

fn handle_active_search_key(state_mut: &mut UblxState, action: keymap::UblxAction) -> bool {
    if !state_mut.search.active {
        return false;
    }
    match action {
        keymap::UblxAction::SearchSubmit => state_mut.search.active = false,
        keymap::UblxAction::SearchBackspace => {
            state_mut.search.query.pop();
        }
        keymap::UblxAction::SearchChar(c) => state_mut.search.query.push(c),
        _ => {}
    }
    keymap::search_consumes(action)
}

fn key_action_context_for(state: &UblxState, tabs: MainTabFlags) -> keymap::KeyActionContext {
    let has_search_filter = !state.search.query.is_empty();
    keymap::KeyActionContext {
        search: keymap::KeySearchState {
            active: state.search.active,
            has_filter: has_search_filter,
        },
        viewer_find: keymap::ViewerFinderBools {
            typing: state.viewer_find.active,
            committed: state.viewer_find.committed,
        },
        allow: keymap::AllowBools {
            viewer_find: !matches!(state.main_mode, MainMode::Settings),
            lens_add_to_other_hotkey: state.main_mode == MainMode::Lenses
                && matches!(state.panels.focus, PanelFocus::Contents)
                && !state.multiselect.active
                && !state.search.active
                && !state.qa_menu.visible
                && !file_ops::modal_open(state)
                && state.lens_confirm.rename_input.is_none()
                && !state.lens_confirm.delete_visible
                && !state.lens_menu.visible
                && !state.multiselect.bulk_menu_visible,
        },
        last_key_for_double: state.last_key_for_double,
        tabs: keymap::KeyOptionalTabs {
            duplicates: tabs.has_duplicates,
            lenses: tabs.has_lenses,
        },
        tab_keys: MAIN_TAB_KEYS,
        multiselect: keymap::MultiselectBools {
            active: state.multiselect.active,
            bulk_menu_visible: state.multiselect.bulk_menu_visible,
            block_bulk_activation: file_ops::modal_open(state)
                || state.lens_confirm.rename_input.is_some()
                || state.lens_confirm.delete_visible
                || state.lens_menu.visible
                || state.main_mode == MainMode::Duplicates,
        },
        panel_focus_contents: matches!(state.panels.focus, PanelFocus::Contents),
        lens_menu_list_open: state.lens_menu.visible && state.lens_menu.name_input.is_none(),
    }
}

/// Yes/no overlays and space-menu letter hotkeys override the mapped action.
fn apply_overlay_key_overrides(
    state_mut: &mut UblxState,
    e: KeyEvent,
    action: &mut keymap::UblxAction,
) {
    let binary_y_n_overlay = state_mut.file_delete_confirm.visible
        || state_mut.lens_confirm.delete_visible
        || state_mut.enhance_policy_menu.visible
        || matches!(
            state_mut.startup_prompt.as_ref().map(|s| &s.phase),
            Some(StartupPromptPhase::PreviousSettings { .. } | StartupPromptPhase::Enhance { .. })
        );
    if binary_y_n_overlay
        && !e.modifiers.contains(KeyModifiers::CONTROL)
        && !e.modifiers.contains(KeyModifiers::SHIFT)
        && let KeyCode::Char(c) = e.code
    {
        match c.to_ascii_lowercase() {
            'y' => {
                *action = keymap::UblxAction::ConfirmYes;
                state_mut.last_key_for_double = None;
            }
            'n' => {
                *action = keymap::UblxAction::ConfirmNo;
                state_mut.last_key_for_double = None;
            }
            _ => {}
        }
    }

    if state_mut.qa_menu.visible
        && let Some(ref kind) = state_mut.qa_menu.kind
        && !e.modifiers.contains(KeyModifiers::CONTROL)
        && !e.modifiers.contains(KeyModifiers::SHIFT)
        && let KeyCode::Char(c) = e.code
        && let Some(idx) = menus::qa_menu_hotkey_to_index(kind, c, state_mut.main_mode)
    {
        *action = keymap::UblxAction::SpaceMenuHotkeySelect(idx);
        state_mut.last_key_for_double = None;
    }

    if state_mut.multiselect.bulk_menu_visible
        && !e.modifiers.contains(KeyModifiers::CONTROL)
        && !e.modifiers.contains(KeyModifiers::SHIFT)
        && let KeyCode::Char(c) = e.code
    {
        match c.to_ascii_lowercase() {
            'a' => {
                *action = keymap::UblxAction::BulkMenuHotkeySelect(0);
                state_mut.last_key_for_double = None;
            }
            'r' => {
                *action = keymap::UblxAction::BulkMenuHotkeySelect(1);
                state_mut.last_key_for_double = None;
            }
            'd' => {
                *action = keymap::UblxAction::BulkMenuHotkeySelect(2);
                state_mut.last_key_for_double = None;
            }
            'z' if state_mut.multiselect.bulk_menu_zahir_row => {
                *action = keymap::UblxAction::BulkMenuHotkeySelect(3);
                state_mut.last_key_for_double = None;
            }
            _ => {}
        }
    }
}

pub fn apply_action_with_settings_enter(
    state_mut: &mut UblxState,
    view_ref: &ViewData,
    right_content_ref: &RightPaneContent,
    action: keymap::UblxAction,
    tabs: MainTabFlags,
    params_mut: &mut RunUblxParams<'_>,
) -> bool {
    let mode_before = state_mut.main_mode;
    let ctx = state_transitions::UblxActionContext::new(view_ref, right_content_ref);
    let quit = ctx.apply_action_to_state(state_mut, action, tabs.has_duplicates, tabs.has_lenses);
    if state_mut.main_mode == MainMode::Settings && mode_before != MainMode::Settings {
        modules::settings::on_enter_settings(state_mut, params_mut);
    }
    quit
}

fn handle_ublx_keyboard(
    state_mut: &mut UblxState,
    e: KeyEvent,
    panes: &ViewPaneRefs<'_>,
    theme_ctx: &modules::theme_selector::ThemeContext,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    tabs: MainTabFlags,
) -> bool {
    if let Some(quit) = ctrl_chord::handle_chord_key_event(
        state_mut,
        &e,
        panes.view,
        panes.right,
        params_mut,
        tabs,
        theme_ctx,
    ) {
        return quit;
    }

    if matches!(e.code, KeyCode::Char(' '))
        && e.modifiers.contains(KeyModifiers::CONTROL)
        && e.kind == KeyEventKind::Press
        && multiselect::try_toggle_mode(state_mut, panes.view, panes.all_rows)
    {
        return false;
    }

    if matches!(e.code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&ublx_opts_mut.command_mode_leader))
        && e.modifiers.contains(KeyModifiers::CONTROL)
        && e.kind == KeyEventKind::Press
        && ctrl_chord::try_begin_chord(state_mut)
    {
        return false;
    }

    let result = keymap::key_action_setup(e, &key_action_context_for(state_mut, tabs));
    state_mut.last_key_for_double = result.last_key_for_double;
    let mut action = result.action;

    apply_overlay_key_overrides(state_mut, e, &mut action);

    if try_open_settings_editor_from_menu(state_mut, action, &*ublx_opts_mut) {
        return false;
    }

    if multiselect::handle_key(
        state_mut,
        panes.view,
        panes.all_rows,
        params_mut,
        ublx_opts_mut,
        action,
    ) {
        return false;
    }

    if dispatch_modal_handlers(
        state_mut,
        panes.view,
        panes.right,
        theme_ctx,
        params_mut,
        ublx_opts_mut,
        ModalInput { e, action },
    ) {
        return false;
    }

    if handle_post_modal_chrome_keys(state_mut, theme_ctx, params_mut, ublx_opts_mut, e, action) {
        return false;
    }

    if handle_viewer_find_typing(state_mut, action) {
        return false;
    }

    if handle_active_search_key(state_mut, action) {
        return false;
    }

    apply_action_with_settings_enter(state_mut, panes.view, panes.right, action, tabs, params_mut)
}

/// Poll for one key event, map it to a [`UblxAction`], run modal handlers, then apply the action to state.
/// Returns `Ok(true)` when the event was handled and the main loop should skip further processing for this tick;
/// `Ok(false)` when there was no key, a non-key event, or the event was consumed without requesting quit semantics.
///
/// # Errors
///
/// Returns [`io::Error`] when `crossterm` fails to poll or read the next event (`event::poll` / `event::read`).
pub fn handle_ublx_input(
    state_mut: &mut UblxState,
    ctx: InputContext<'_>,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
) -> io::Result<bool> {
    let InputContext {
        view: view_ref,
        all_rows,
        right_content: right_content_ref,
        theme_ctx,
        frame_area,
        layout,
        tabs,
    } = ctx;
    if !event::poll(std::time::Duration::from_millis(UI_CONSTANTS.input_poll_ms))? {
        return Ok(false);
    }
    let ev = event::read()?;
    if let Event::Mouse(me) = ev {
        let _handled = mouse::handle_mouse_event(
            state_mut,
            me,
            mouse::MouseContext {
                view: view_ref,
                right_content: right_content_ref,
                frame_area,
                layout,
                tabs,
                main_mode: state_mut.main_mode,
                command_mode_leader: ublx_opts_mut.command_mode_leader,
            },
        );
        return Ok(false);
    }
    let Event::Key(e) = ev else {
        return Ok(false);
    };

    let panes = ViewPaneRefs {
        view: view_ref,
        all_rows,
        right: right_content_ref,
    };
    Ok(handle_ublx_keyboard(
        state_mut,
        e,
        &panes,
        &theme_ctx,
        params_mut,
        ublx_opts_mut,
        tabs,
    ))
}
