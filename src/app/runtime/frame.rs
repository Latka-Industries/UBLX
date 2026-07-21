//! Build [`DrawFrameArgs`] and draw one terminal frame.

use std::io;

use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;

use crate::app::RunUblxParams;
use crate::layout::setup;
use crate::render::{DrawFrameArgs, draw_ublx_frame};
use crate::themes;
use crate::utils::OPACITY_SOLID_MIN;

/// Inputs for building draw args and drawing one frame. Built once per tick and reused for the normal draw and optional post-editor refresh.
pub struct DrawInputs<'a> {
    pub params: &'a RunUblxParams<'a>,
    pub delta_data: Option<&'a setup::DeltaViewData>,
    pub rows_for_draw: Option<&'a [setup::TuiRow]>,
    pub theme_name: Option<&'a str>,
    pub last_snapshot_ns: Option<i64>,
}

/// Draw one frame using current view and right content. Used for the normal tick draw and for the post-editor refresh.
pub fn draw_one_frame(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut setup::UblxState,
    view: &setup::ViewData,
    right_content: &setup::RightPaneContent,
    draw_inputs: &DrawInputs<'_>,
) -> io::Result<()> {
    let draw_args = build_draw_args(
        draw_inputs.params,
        draw_inputs.delta_data,
        draw_inputs.rows_for_draw,
        draw_inputs.theme_name,
        draw_inputs.last_snapshot_ns,
    );
    terminal
        .draw(|f| draw_ublx_frame(f, state, view, right_content, &draw_args))
        .map(|_| ())
}

/// Build [`DrawFrameArgs`] from params and per-tick values.
fn build_draw_args<'a>(
    params: &'a RunUblxParams<'_>,
    delta_data: Option<&'a setup::DeltaViewData>,
    rows_for_draw: Option<&'a [setup::TuiRow]>,
    theme_name: Option<&'a str>,
    last_snapshot_ns: Option<i64>,
) -> DrawFrameArgs<'a> {
    DrawFrameArgs {
        delta_data,
        all_rows: rows_for_draw,
        dir_to_ublx: Some(params.dir_to_ublx.as_path()),
        theme_name,
        layout: &params.layout,
        bg_opacity: params.bg_opacity,
        transparent_page_chrome: params.bg_opacity < OPACITY_SOLID_MIN,
        last_snapshot_ns,
        dev: params.display.dev,
        duplicate_groups: if params.duplicate_groups.is_empty() {
            None
        } else {
            Some(params.duplicate_groups.as_slice())
        },
        duplicate_mode: params.duplicate_mode,
        lens_names: if params.lens_names.is_empty() {
            None
        } else {
            Some(params.lens_names.as_slice())
        },
        command_mode_leader: params.command_mode_leader,
    }
}

/// Return owned theme name so callers don't hold a borrow of state (avoids borrow conflicts with draw/input).
pub fn theme_name_for_tick(state: &setup::UblxState, params: &RunUblxParams<'_>) -> Option<String> {
    if state.theme.selector_visible {
        Some(
            themes::theme_ordered_list()[state.theme.selector_index]
                .name
                .to_string(),
        )
    } else {
        state
            .theme
            .override_name
            .clone()
            .or_else(|| params.theme.clone())
    }
}
