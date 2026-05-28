//! Settings tab: enter, key dispatch (`handle_key`), and orchestration. Bool rows and layout edit live in
//! [`super::bool_rows`] and [`super::layout_edit`]; path/overlay sync in [`super::context`].

use std::path::Path;

use crate::app::RunUblxParams;
use crate::config::{
    LayoutOverlay, Osc11BackgroundFormat, UblxOpts, UblxOverlay, UblxPaths, load_ublx_toml,
    strip_global_only_keys_from_local_overlay, write_ublx_overlay_at,
};
use crate::handlers::state_transitions::PREVIEW_SCROLL_STEP_LINES;
use crate::layout::setup::{SettingsConfigScope, UblxState};
use crate::ui::{UblxAction, show_operation_toast};
use crate::utils::{clamp_selection, opacity_is_solid};

use super::apply_config_reload;
use super::bool_rows;
use super::context;
use super::layout_edit;
use super::typed_column_tables_row;

pub fn on_enter_settings(state_mut: &mut UblxState, params_ref: &RunUblxParams<'_>) {
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

fn persist_overlay_reload_refresh(
    path: &Path,
    overlay: &UblxOverlay,
    scope: SettingsConfigScope,
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    after_apply: impl FnOnce(&mut UblxState),
) {
    let mut to_write = overlay.clone();
    if scope == SettingsConfigScope::Local {
        strip_global_only_keys_from_local_overlay(&mut to_write);
    }
    write_ublx_overlay_at(path, &to_write);
    state_mut.config_written_by_us_at = Some(std::time::Instant::now());
    apply_config_reload(params_mut, ublx_opts_mut, state_mut, None::<&str>);
    after_apply(state_mut);
    context::refresh_editing_metadata(state_mut, params_mut);
}

fn submit_settings_bool_row(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    scope: SettingsConfigScope,
    cur: usize,
) {
    let Some(path) = state_mut.settings.editing_path.clone() else {
        return;
    };
    let paths = UblxPaths::new(&params_mut.dir_to_ublx);
    let mut overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
    let merged_before = context::merged_overlay_before_write(&paths, scope, &overlay);
    let v = !bool_rows::overlay_bool(&merged_before, scope, cur);
    bool_rows::write_bool(&mut overlay, scope, cur, v);
    persist_overlay_reload_refresh(
        path.as_path(),
        &overlay,
        scope,
        state_mut,
        params_mut,
        ublx_opts_mut,
        |_| {},
    );
    let label = bool_rows::bool_row_label(scope, cur, false);
    let msg = format!("{label} = {v}");
    show_operation_toast(
        state_mut,
        params_mut,
        msg,
        "settings-bool",
        log::Level::Info,
    );
}

fn submit_settings_typed_column_tables_row(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    scope: SettingsConfigScope,
) {
    let Some(path) = state_mut.settings.editing_path.clone() else {
        return;
    };
    let paths = UblxPaths::new(&params_mut.dir_to_ublx);
    let mut overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
    let merged_before = context::merged_overlay_before_write(&paths, scope, &overlay);
    let current = typed_column_tables_row::overlay_typed_column_tables(&merged_before);
    let next = typed_column_tables_row::cycle_typed_column_tables(current);
    typed_column_tables_row::write_typed_column_tables(&mut overlay, next);
    persist_overlay_reload_refresh(
        path.as_path(),
        &overlay,
        scope,
        state_mut,
        params_mut,
        ublx_opts_mut,
        |_| {},
    );
    show_operation_toast(
        state_mut,
        params_mut,
        format!(
            "typed_column_tables = {}",
            typed_column_tables_row::typed_column_tables_toml_value(next)
        ),
        "settings-typed-column-tables",
        log::Level::Info,
    );
}

fn submit_settings_opacity_format_row(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    scope: SettingsConfigScope,
) {
    let Some(path) = state_mut.settings.editing_path.clone() else {
        return;
    };
    let paths = UblxPaths::new(&params_mut.dir_to_ublx);
    let mut overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
    let merged_before = context::merged_overlay_before_write(&paths, scope, &overlay);
    let current = merged_before.opacity_format.unwrap_or_default();
    let next = match current {
        Osc11BackgroundFormat::Rgba => Osc11BackgroundFormat::Hex8,
        Osc11BackgroundFormat::Hex8 => Osc11BackgroundFormat::Rgba,
    };
    overlay.opacity_format = Some(next);
    persist_overlay_reload_refresh(
        path.as_path(),
        &overlay,
        scope,
        state_mut,
        params_mut,
        ublx_opts_mut,
        |_| {},
    );
    let fmt_label = match next {
        Osc11BackgroundFormat::Rgba => "rgba",
        Osc11BackgroundFormat::Hex8 => "hex8",
    };
    show_operation_toast(
        state_mut,
        params_mut,
        format!("opacity_format = {fmt_label}"),
        "settings-opacity-format",
        log::Level::Info,
    );
}

fn submit_settings_layout_row(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    scope: SettingsConfigScope,
    layout_btn_index: usize,
) {
    if state_mut.settings.layout_unlocked {
        let Some(path) = state_mut.settings.editing_path.clone() else {
            return;
        };
        let Some((l, m, r)) = layout_edit::parse_layout_triplet(
            &state_mut.settings.layout_left_buf,
            &state_mut.settings.layout_mid_buf,
            &state_mut.settings.layout_right_buf,
        ) else {
            show_operation_toast(
                state_mut,
                params_mut,
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
        persist_overlay_reload_refresh(
            path.as_path(),
            &overlay,
            scope,
            state_mut,
            params_mut,
            ublx_opts_mut,
            |s| {
                s.settings.layout_unlocked = false;
            },
        );
        state_mut.settings.left_cursor = clamp_selection(
            layout_btn_index,
            layout_edit::left_cursor_len(&state_mut.settings, scope),
        );
        show_operation_toast(
            state_mut,
            params_mut,
            format!("layout {l}/{m}/{r}"),
            "settings-layout",
            log::Level::Info,
        );
    } else {
        state_mut.settings.layout_unlocked = true;
        let paths = UblxPaths::new(&params_mut.dir_to_ublx);
        if scope == SettingsConfigScope::Local {
            let (local_o, merged) = context::local_edit_context(&paths);
            let lay_src = context::layout_overlay_for_local_editing(local_o.as_ref(), &merged);
            context::sync_layout_buffers_from_overlay(&mut state_mut.settings, lay_src);
        } else if let Some(path) = state_mut.settings.editing_path.clone()
            && let Some(overlay) = load_ublx_toml(Some(path), None)
        {
            context::sync_layout_buffers_from_overlay(&mut state_mut.settings, &overlay);
        }
        state_mut.settings.left_cursor =
            (layout_btn_index + 1).min(layout_edit::max_left_cursor(&state_mut.settings, scope));
    }
}

fn submit_settings_bg_opacity_row(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    scope: SettingsConfigScope,
    op_btn: usize,
) {
    if state_mut.settings.opacity_unlocked {
        let Some(path) = state_mut.settings.editing_path.clone() else {
            return;
        };
        let Some(v) = layout_edit::parse_bg_opacity(&state_mut.settings.opacity_buf) else {
            show_operation_toast(
                state_mut,
                params_mut,
                "bg_opacity: enter a number from 0.0 to 1.0",
                "settings-opacity",
                log::Level::Warn,
            );
            return;
        };
        let mut overlay = load_ublx_toml(Some(path.clone()), None).unwrap_or_default();
        overlay.bg_opacity = if opacity_is_solid(v) { None } else { Some(v) };
        persist_overlay_reload_refresh(
            path.as_path(),
            &overlay,
            scope,
            state_mut,
            params_mut,
            ublx_opts_mut,
            |s| {
                s.settings.opacity_unlocked = false;
            },
        );
        state_mut.settings.left_cursor = clamp_selection(
            op_btn,
            layout_edit::left_cursor_len(&state_mut.settings, scope),
        );
        show_operation_toast(
            state_mut,
            params_mut,
            format!("bg_opacity = {v}"),
            "settings-opacity",
            log::Level::Info,
        );
    } else {
        state_mut.settings.opacity_unlocked = true;
        let paths = UblxPaths::new(&params_mut.dir_to_ublx);
        if scope == SettingsConfigScope::Local {
            let (local_o, merged) = context::local_edit_context(&paths);
            let op_src = context::opacity_overlay_for_local_editing(local_o.as_ref(), &merged);
            context::sync_opacity_buffer_from_overlay(&mut state_mut.settings, op_src);
        } else if let Some(path) = state_mut.settings.editing_path.clone()
            && let Some(overlay) = load_ublx_toml(Some(path), None)
        {
            context::sync_opacity_buffer_from_overlay(&mut state_mut.settings, &overlay);
        }
        state_mut.settings.left_cursor =
            (op_btn + 1).min(layout_edit::max_left_cursor(&state_mut.settings, scope));
    }
}

/// Enter on Search: toggle bool row, or layout row (unlock / save).
fn handle_settings_search_submit(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    scope: SettingsConfigScope,
) {
    let layout_btn = layout_edit::layout_button_index(scope);
    let cur = state_mut.settings.left_cursor;
    if cur < bool_rows::bool_row_count(scope) {
        submit_settings_bool_row(state_mut, params_mut, ublx_opts_mut, scope, cur);
    } else if cur == typed_column_tables_row::typed_column_tables_row_index(scope) {
        submit_settings_typed_column_tables_row(state_mut, params_mut, ublx_opts_mut, scope);
    } else if Some(cur) == layout_edit::opacity_format_row_index(scope) {
        submit_settings_opacity_format_row(state_mut, params_mut, ublx_opts_mut, scope);
    } else if cur == layout_btn {
        submit_settings_layout_row(state_mut, params_mut, ublx_opts_mut, scope, layout_btn);
    } else if cur == layout_edit::opacity_button_index(&state_mut.settings, scope) {
        submit_settings_bg_opacity_row(state_mut, params_mut, ublx_opts_mut, scope, cur);
    }
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
            handle_settings_search_submit(state_mut, params_mut, ublx_opts_mut, scope);
            true
        }
        _ => false,
    }
}
