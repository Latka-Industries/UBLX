//! Settings tab: enter, key dispatch (`handle_key`), and orchestration. Bool rows and layout edit live in
//! [`super::bool_rows`] and [`super::layout_edit`]; path/overlay sync in [`super::context`].

use std::path::{Path, PathBuf};

use crate::app::RunUblxParams;
use crate::config::{
    LayoutOverlay, OPERATION_NAME, Osc11BackgroundFormat, UblxOpts, UblxOverlay, UblxPaths,
    command_mode_leader_reject_reason, cycle_command_mode_leader, load_ublx_toml,
    strip_global_only_keys_from_local_overlay, write_command_mode_leader, write_ublx_overlay_at,
};
use crate::handlers::state_transitions::PREVIEW_SCROLL_STEP_LINES;
use crate::layout::setup::{SettingsConfigScope, UblxState};
use crate::ui::{UblxAction, end_chord, show_operation_toast};
use crate::utils::{clamp_selection, opacity_is_solid};

use super::apply_config_reload;
use super::bool_rows;
use super::command_mode_leader_row;
use super::context;
use super::layout_edit;
use super::typed_column_tables_row;

/// Shared mutable handles for Settings submit / key handlers.
struct SettingsEditCtx<'a, 'p> {
    state: &'a mut UblxState,
    params: &'a mut RunUblxParams<'p>,
    opts: &'a mut UblxOpts,
}

impl<'a, 'p> SettingsEditCtx<'a, 'p> {
    fn new(
        state: &'a mut UblxState,
        params: &'a mut RunUblxParams<'p>,
        opts: &'a mut UblxOpts,
    ) -> Self {
        Self {
            state,
            params,
            opts,
        }
    }

    fn scope(&self) -> SettingsConfigScope {
        self.state.settings.scope
    }

    fn toast(&mut self, message: impl AsRef<str>, op_suffix: &str, level: log::Level) {
        show_operation_toast(self.state, self.params, message, op_suffix, level);
    }

    fn toast_fresh(&mut self, message: impl AsRef<str>, op_suffix: &str, level: log::Level) {
        let op = OPERATION_NAME.op(op_suffix);
        self.state.toasts.consumed_per_operation.remove(&op);
        self.toast(message, op_suffix, level);
    }

    fn persist(
        &mut self,
        path: &Path,
        overlay: &UblxOverlay,
        after_apply: impl FnOnce(&mut UblxState),
    ) {
        let scope = self.scope();
        let mut to_write = overlay.clone();
        if scope == SettingsConfigScope::Local {
            strip_global_only_keys_from_local_overlay(&mut to_write);
        }
        write_ublx_overlay_at(path, &to_write);
        self.state.config_written_by_us_at = Some(std::time::Instant::now());
        apply_config_reload(self.params, self.opts, self.state, None::<&str>);
        after_apply(self.state);
        context::refresh_editing_metadata(self.state, self.params);
    }

    /// `(path, file overlay, merged-before-write view)` for the active editing path.
    fn load_edit_overlays(&self) -> Option<(PathBuf, UblxOverlay, UblxOverlay)> {
        let path = self.state.settings.editing_path.clone()?;
        let overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
        let paths = UblxPaths::new(&self.params.dir_to_ublx);
        let merged = context::merged_overlay_before_write(&paths, self.scope(), &overlay);
        Some((path, overlay, merged))
    }

    fn submit_bool_row(&mut self, cur: usize) {
        let scope = self.scope();
        let Some((path, mut overlay, merged_before)) = self.load_edit_overlays() else {
            return;
        };
        let v = !bool_rows::overlay_bool(&merged_before, scope, cur);
        bool_rows::write_bool(&mut overlay, scope, cur, v);
        self.persist(path.as_path(), &overlay, |_| {});
        let label = bool_rows::bool_row_label(scope, cur, false);
        self.toast(format!("{label} = {v}"), "settings-bool", log::Level::Info);
    }

    fn submit_command_mode_leader(&mut self, next: char) {
        let Some((path, mut overlay, _)) = self.load_edit_overlays() else {
            return;
        };
        if self.opts.command_mode_leader == next {
            self.toast_fresh(
                format!("command_mode.leader = \"{next}\" (unchanged)"),
                command_mode_leader_row::LEADER_TOAST_OP,
                log::Level::Info,
            );
            return;
        }
        write_command_mode_leader(&mut overlay, next);
        self.persist(path.as_path(), &overlay, |_| {});
        self.toast_fresh(
            format!("command_mode.leader = \"{next}\""),
            command_mode_leader_row::LEADER_TOAST_OP,
            log::Level::Info,
        );
    }

    fn cycle_command_mode_leader(&mut self) {
        let next = cycle_command_mode_leader(self.opts.command_mode_leader);
        self.submit_command_mode_leader(next);
    }

    /// Load buffers for the row being unlocked, then move the cursor onto the edit fields.
    fn unlock_numeric_row(&mut self, btn_index: usize, layout: bool) {
        let scope = self.scope();
        if layout {
            self.state.settings.layout_unlocked = true;
        } else {
            self.state.settings.opacity_unlocked = true;
        }
        let paths = UblxPaths::new(&self.params.dir_to_ublx);
        if scope == SettingsConfigScope::Local {
            let (local_o, merged) = context::local_edit_context(&paths);
            if layout {
                let src = context::layout_overlay_for_local_editing(local_o.as_ref(), &merged);
                context::sync_layout_buffers_from_overlay(&mut self.state.settings, src);
            } else {
                let src = context::opacity_overlay_for_local_editing(local_o.as_ref(), &merged);
                context::sync_opacity_buffer_from_overlay(&mut self.state.settings, src);
            }
        } else if let Some(path) = self.state.settings.editing_path.clone()
            && let Some(overlay) = load_ublx_toml(Some(path), None)
        {
            if layout {
                context::sync_layout_buffers_from_overlay(&mut self.state.settings, &overlay);
            } else {
                context::sync_opacity_buffer_from_overlay(&mut self.state.settings, &overlay);
            }
        }
        self.state.settings.left_cursor =
            (btn_index + 1).min(layout_edit::max_left_cursor(&self.state.settings, scope));
    }

    fn submit_typed_column_tables(&mut self) {
        let Some((path, mut overlay, merged_before)) = self.load_edit_overlays() else {
            return;
        };
        let current = typed_column_tables_row::overlay_typed_column_tables(&merged_before);
        let next = typed_column_tables_row::cycle_typed_column_tables(current);
        typed_column_tables_row::write_typed_column_tables(&mut overlay, next);
        self.persist(path.as_path(), &overlay, |_| {});
        self.toast(
            format!(
                "typed_column_tables = {}",
                typed_column_tables_row::typed_column_tables_toml_value(next)
            ),
            "settings-typed-column-tables",
            log::Level::Info,
        );
    }

    fn submit_opacity_format(&mut self) {
        let Some((path, mut overlay, merged_before)) = self.load_edit_overlays() else {
            return;
        };
        let current = merged_before.opacity_format.unwrap_or_default();
        let next = match current {
            Osc11BackgroundFormat::Rgba => Osc11BackgroundFormat::Hex8,
            Osc11BackgroundFormat::Hex8 => Osc11BackgroundFormat::Rgba,
        };
        overlay.opacity_format = Some(next);
        self.persist(path.as_path(), &overlay, |_| {});
        let fmt_label = match next {
            Osc11BackgroundFormat::Rgba => "rgba",
            Osc11BackgroundFormat::Hex8 => "hex8",
        };
        self.toast(
            format!("opacity_format = {fmt_label}"),
            "settings-opacity-format",
            log::Level::Info,
        );
    }

    fn submit_layout_row(&mut self, layout_btn_index: usize) {
        let scope = self.scope();
        if self.state.settings.layout_unlocked {
            let Some(path) = self.state.settings.editing_path.clone() else {
                return;
            };
            let Some((l, m, r)) = layout_edit::parse_layout_triplet(
                &self.state.settings.layout_left_buf,
                &self.state.settings.layout_mid_buf,
                &self.state.settings.layout_right_buf,
            ) else {
                self.toast(
                    "layout: three u8 must sum to 100",
                    "settings-layout",
                    log::Level::Warn,
                );
                return;
            };
            let mut overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
            overlay.layout = Some(LayoutOverlay {
                left_pct: l,
                middle_pct: m,
                right_pct: r,
            });
            self.persist(path.as_path(), &overlay, |s| {
                s.settings.layout_unlocked = false;
            });
            self.state.settings.left_cursor = clamp_selection(
                layout_btn_index,
                layout_edit::left_cursor_len(&self.state.settings, scope),
            );
            self.toast(
                format!("layout {l}/{m}/{r}"),
                "settings-layout",
                log::Level::Info,
            );
        } else {
            self.unlock_numeric_row(layout_btn_index, true);
        }
    }

    fn submit_bg_opacity_row(&mut self, op_btn: usize) {
        let scope = self.scope();
        if self.state.settings.opacity_unlocked {
            let Some(path) = self.state.settings.editing_path.clone() else {
                return;
            };
            let Some(v) = layout_edit::parse_bg_opacity(&self.state.settings.opacity_buf) else {
                self.toast(
                    "bg_opacity: enter a number from 0.0 to 1.0",
                    "settings-opacity",
                    log::Level::Warn,
                );
                return;
            };
            let mut overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
            overlay.bg_opacity = if opacity_is_solid(v) { None } else { Some(v) };
            self.persist(path.as_path(), &overlay, |s| {
                s.settings.opacity_unlocked = false;
            });
            self.state.settings.left_cursor = clamp_selection(
                op_btn,
                layout_edit::left_cursor_len(&self.state.settings, scope),
            );
            self.toast(
                format!("bg_opacity = {v}"),
                "settings-opacity",
                log::Level::Info,
            );
        } else {
            self.unlock_numeric_row(op_btn, false);
        }
    }

    fn handle_search_submit(&mut self) {
        let scope = self.scope();
        let layout_btn = layout_edit::layout_button_index(scope);
        let cur = self.state.settings.left_cursor;
        if cur < bool_rows::bool_row_count(scope) {
            self.submit_bool_row(cur);
        } else if cur == typed_column_tables_row::typed_column_tables_row_index(scope) {
            self.submit_typed_column_tables();
        } else if Some(cur) == command_mode_leader_row::command_mode_leader_row_index(scope) {
            self.cycle_command_mode_leader();
        } else if Some(cur) == layout_edit::opacity_format_row_index(scope) {
            self.submit_opacity_format();
        } else if cur == layout_btn {
            self.submit_layout_row(layout_btn);
        } else if cur == layout_edit::opacity_button_index(&self.state.settings, scope) {
            self.submit_bg_opacity_row(cur);
        }
    }
}

pub fn on_enter_settings(state_mut: &mut UblxState, params_ref: &RunUblxParams<'_>) {
    end_chord(state_mut);
    state_mut.settings.left_cursor = 0;
    state_mut.settings.right_scroll = 0;
    state_mut.settings.layout_unlocked = false;
    state_mut.settings.opacity_unlocked = false;
    context::refresh_editing_metadata(state_mut, params_ref);
    let scope = state_mut.settings.scope;
    state_mut.settings.left_cursor = clamp_selection(
        state_mut.settings.left_cursor,
        layout_edit::left_cursor_len(&state_mut.settings, scope),
    );
}

/// When the Command Mode leader row is focused, a plain letter key sets the leader.
pub fn handle_command_mode_leader_key(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    key: crossterm::event::KeyEvent,
) -> bool {
    use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

    if key.kind != KeyEventKind::Press {
        return false;
    }
    let scope = state_mut.settings.scope;
    let Some(row) = command_mode_leader_row::command_mode_leader_row_index(scope) else {
        return false;
    };
    if state_mut.settings.left_cursor != row {
        return false;
    }
    let KeyCode::Char(c) = key.code else {
        return false;
    };
    if !key.modifiers.intersection(KeyModifiers::CONTROL).is_empty() {
        return false;
    }
    if !c.is_ascii_alphabetic() {
        return false;
    }
    let c = c.to_ascii_lowercase();
    let mut ctx = SettingsEditCtx::new(state_mut, params_mut, ublx_opts_mut);
    if let Some(reason) = command_mode_leader_reject_reason(c) {
        ctx.toast_fresh(
            reason,
            command_mode_leader_row::LEADER_TOAST_OP,
            log::Level::Warn,
        );
        return true;
    }
    ctx.submit_command_mode_leader(c);
    true
}

/// Handle a mapped action while on the Settings tab. Returns true if the key should not propagate.
#[must_use]
pub fn handle_key(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    action: UblxAction,
) -> bool {
    let scope = state_mut.settings.scope;

    match action {
        UblxAction::Tab => {
            state_mut.settings.scope = match state_mut.settings.scope {
                SettingsConfigScope::Global => SettingsConfigScope::Local,
                SettingsConfigScope::Local => SettingsConfigScope::Global,
            };
            state_mut.settings.layout_unlocked = false;
            state_mut.settings.opacity_unlocked = false;
            context::refresh_editing_metadata(state_mut, params_mut);
            let sc = state_mut.settings.scope;
            state_mut.settings.left_cursor = clamp_selection(
                state_mut.settings.left_cursor,
                layout_edit::left_cursor_len(&state_mut.settings, sc),
            );
            true
        }
        UblxAction::ScrollPreviewUp => {
            state_mut.settings.right_scroll = state_mut
                .settings
                .right_scroll
                .saturating_sub(PREVIEW_SCROLL_STEP_LINES);
            true
        }
        UblxAction::ScrollPreviewDown => {
            state_mut.settings.right_scroll = state_mut
                .settings
                .right_scroll
                .saturating_add(PREVIEW_SCROLL_STEP_LINES);
            true
        }
        UblxAction::PreviewTop => {
            state_mut.settings.right_scroll = 0;
            true
        }
        UblxAction::PreviewBottom => {
            state_mut.settings.right_scroll = u16::MAX;
            true
        }
        UblxAction::MoveDown | UblxAction::MoveDownFast => {
            layout_edit::bump_settings_cursor(state_mut, scope, true);
            true
        }
        UblxAction::MoveUp | UblxAction::MoveUpFast => {
            layout_edit::bump_settings_cursor(state_mut, scope, false);
            true
        }
        UblxAction::FocusCategories
        | UblxAction::FocusContents
        | UblxAction::ListTop
        | UblxAction::ListBottom
        | UblxAction::CycleContentSort
        | UblxAction::CycleRightPane
        | UblxAction::RightPaneViewer
        | UblxAction::RightPaneTemplates
        | UblxAction::RightPaneMetadata
        | UblxAction::RightPaneWriting
        | UblxAction::ViewerFullscreenToggle => true,
        UblxAction::SearchSubmit => {
            SettingsEditCtx::new(state_mut, params_mut, ublx_opts_mut).handle_search_submit();
            true
        }
        _ => false,
    }
}
