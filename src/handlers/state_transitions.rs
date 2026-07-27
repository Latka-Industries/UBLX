//! Apply key actions to TUI state. Moved from layout so "what happens on key" lives with other behavior.

use crate::integrations::ZahirFT;
use crate::layout::setup::{
    MainMode, PanelFocus, RightPaneContent, RightPaneMode, UblxState, ViewData,
};
use crate::modules::viewer_search;
use crate::ui::UblxAction;
use crate::utils::clamp_selection;

pub const PREVIEW_SCROLL_STEP_LINES: u16 = 5;
pub const LIST_FAST_STEP_ROWS: usize = 10;

/// Quit the application
fn apply_quit(state_mut: &mut UblxState) -> bool {
    if state_mut.chrome.viewer_fullscreen {
        state_mut.chrome.viewer_fullscreen = false;
        false
    } else {
        true
    }
}

/// Apply miscellaneous actions to state
fn apply_misc(state_mut: &mut UblxState, action: UblxAction) {
    match action {
        UblxAction::Help => {
            state_mut.chrome.help_visible = true;
            state_mut.chrome.help_tab = 0;
        }
        UblxAction::TakeSnapshot => state_mut.snapshot_bg.requested = true,
        _ => {}
    }
}

/// Apply mode switch actions to state
fn apply_mode_switch(
    state_mut: &mut UblxState,
    action: UblxAction,
    has_duplicates: bool,
    has_lenses: bool,
) {
    state_mut.multiselect.clear();
    match action {
        UblxAction::MainModeSnapshot => state_mut.main_mode = MainMode::Snapshot,
        UblxAction::MainModeDelta => state_mut.main_mode = MainMode::Delta,
        UblxAction::MainModeSettings => state_mut.main_mode = MainMode::Settings,
        UblxAction::MainModeDuplicates => state_mut.main_mode = MainMode::Duplicates,
        UblxAction::MainModeLenses => state_mut.main_mode = MainMode::Lenses,
        UblxAction::LoadDuplicates => state_mut.duplicate_load.requested = true,
        UblxAction::ExportZahirJson => state_mut.zahir_export_load.requested = true,
        UblxAction::ExportLensMarkdown => state_mut.lens_export_load.requested = true,
        UblxAction::MainModeToggle => {
            state_mut.main_mode = state_mut.main_mode.next(has_duplicates, has_lenses);
        }
        _ => {}
    }
}

/// Apply preview scroll actions to state
fn apply_preview_scroll(state_mut: &mut UblxState, action: UblxAction) {
    let step = PREVIEW_SCROLL_STEP_LINES;
    match action {
        UblxAction::ScrollPreviewUp => {
            state_mut.panels.preview_scroll = state_mut.panels.preview_scroll.saturating_sub(step);
        }
        UblxAction::ScrollPreviewDown => {
            state_mut.panels.preview_scroll = state_mut.panels.preview_scroll.saturating_add(step);
        }
        UblxAction::PreviewTop => state_mut.panels.preview_scroll = 0,
        UblxAction::PreviewBottom => state_mut.panels.preview_scroll = u16::MAX,
        _ => {}
    }
}

/// Templates tab with structured views:
/// Shift+J/K move pattern selection; Shift+B/E scroll the inline examples block.
fn apply_template_list_nav(
    state_mut: &mut UblxState,
    right_content_ref: &RightPaneContent,
    action: UblxAction,
) -> bool {
    if state_mut.right_pane_mode != RightPaneMode::Templates
        || right_content_ref.template_views.is_empty()
    {
        return false;
    }
    match action {
        UblxAction::ScrollPreviewDown | UblxAction::ScrollPreviewUp => {
            let n = right_content_ref.template_views.len();
            let cur = state_mut.panels.template_list.selected().unwrap_or(0);
            let next = if action == UblxAction::ScrollPreviewDown {
                (cur + 1).min(n.saturating_sub(1))
            } else {
                cur.saturating_sub(1)
            };
            if next != cur {
                state_mut.panels.template_list.select(Some(next));
                state_mut.panels.preview_scroll = 0;
            }
            true
        }
        UblxAction::PreviewTop | UblxAction::PreviewBottom => {
            // Inner scroll for the framed examples under the selected pattern.
            apply_preview_scroll(state_mut, action);
            true
        }
        _ => false,
    }
}

/// Check if PDF page navigation applies
fn pdf_page_nav_applies(state_ref: &UblxState, right_content_ref: &RightPaneContent) -> bool {
    state_ref.right_pane_mode == RightPaneMode::Viewer
        && right_content_ref.zahir_file_type() == Some(ZahirFT::Pdf)
        && right_content_ref.derived.abs_path.is_some()
}

/// Apply PDF page scroll actions to state
fn apply_pdf_page_scroll(state_mut: &mut UblxState, action: UblxAction) {
    let max = state_mut.viewer_image.pdf.page_count;
    match action {
        UblxAction::ScrollPreviewDown => {
            let next = state_mut.viewer_image.pdf.page.saturating_add(1);
            state_mut.viewer_image.pdf.page = if let Some(m) = max { next.min(m) } else { next };
        }
        UblxAction::ScrollPreviewUp => {
            state_mut.viewer_image.pdf.page =
                state_mut.viewer_image.pdf.page.saturating_sub(1).max(1);
        }
        UblxAction::PreviewTop => {
            state_mut.viewer_image.pdf.page = 1;
        }
        UblxAction::PreviewBottom => {
            if let Some(m) = max {
                state_mut.viewer_image.pdf.page = m;
            }
        }
        _ => {}
    }
}

/// Context (view + right-pane content) required to apply actions to state.
pub struct UblxActionContext<'a> {
    view_ref: &'a ViewData,
    right_content_ref: &'a RightPaneContent,
}

impl<'a> UblxActionContext<'a> {
    fn selected_content_anchor(&self, state_ref: &UblxState) -> Option<String> {
        self.right_content_ref.snap_meta.path.clone().or_else(|| {
            state_ref.panels.content_state.selected().and_then(|i| {
                self.view_ref
                    .row_at(i, None)
                    .map(|(path, _, _)| path.clone())
            })
        })
    }

    #[must_use]
    pub fn new(view_ref: &'a ViewData, right_content_ref: &'a RightPaneContent) -> Self {
        Self {
            view_ref,
            right_content_ref,
        }
    }

    /// Apply the key action to state (mutates focus, selection, panes, etc.).
    /// Returns true if the user requested quit (caller should exit the run loop).
    /// `has_duplicates` / `has_lenses` are used for `MainModeToggle` and tab keys (cycle / switch only when tab exists).
    pub fn apply_action_to_state(
        &self,
        state_mut: &mut UblxState,
        action: UblxAction,
        has_duplicates: bool,
        has_lenses: bool,
    ) -> bool {
        if let UblxAction::Quit = action {
            return apply_quit(state_mut);
        }
        match action {
            UblxAction::Help | UblxAction::TakeSnapshot => apply_misc(state_mut, action),
            UblxAction::MainModeSnapshot
            | UblxAction::MainModeDelta
            | UblxAction::MainModeSettings
            | UblxAction::MainModeDuplicates
            | UblxAction::MainModeLenses
            | UblxAction::MainModeToggle
            | UblxAction::LoadDuplicates
            | UblxAction::ExportZahirJson
            | UblxAction::ExportLensMarkdown => {
                apply_mode_switch(state_mut, action, has_duplicates, has_lenses);
            }
            UblxAction::SearchStart => {
                viewer_search::clear(state_mut);
                state_mut.search.active = true;
            }
            UblxAction::ViewerFindOpen => {
                state_mut.search.active = false;
                state_mut.viewer_find.active = true;
                state_mut.viewer_find.committed = false;
            }
            UblxAction::ViewerFindNext => {
                let vf = &mut state_mut.viewer_find;
                if !vf.ranges.is_empty() {
                    vf.current = (vf.current + 1) % vf.ranges.len();
                    vf.pending_scroll = true;
                }
            }
            UblxAction::ViewerFindPrev => {
                let vf = &mut state_mut.viewer_find;
                if !vf.ranges.is_empty() {
                    vf.current = vf
                        .current
                        .checked_sub(1)
                        .unwrap_or(vf.ranges.len().saturating_sub(1));
                    vf.pending_scroll = true;
                }
            }
            UblxAction::CycleRightPane
            | UblxAction::RightPaneViewer
            | UblxAction::ViewerFullscreenToggle
            | UblxAction::RightPaneTemplates
            | UblxAction::RightPaneMetadata
            | UblxAction::RightPaneWriting => self.apply_right_pane(state_mut, action),
            UblxAction::ScrollPreviewUp
            | UblxAction::ScrollPreviewDown
            | UblxAction::PreviewTop
            | UblxAction::PreviewBottom => {
                if pdf_page_nav_applies(state_mut, self.right_content_ref) {
                    apply_pdf_page_scroll(state_mut, action);
                } else if apply_template_list_nav(state_mut, self.right_content_ref, action) {
                    // Templates: Shift+J/K = patterns; Shift+B/E = examples scroll.
                } else {
                    apply_preview_scroll(state_mut, action);
                }
            }
            UblxAction::CycleContentSort => {
                state_mut.panels.sort_anchor_path = self.selected_content_anchor(state_mut);
                state_mut.panels.content_sort = state_mut
                    .panels
                    .content_sort
                    .cycle_for_mode(state_mut.main_mode);
            }
            UblxAction::AddToOtherLens => {
                if state_mut.main_mode == MainMode::Lenses
                    && let Some(i) = state_mut.panels.content_state.selected()
                    && let Some(row) = self.view_ref.row_at(i, None)
                {
                    let path = row.0.clone();
                    let ex = self
                        .view_ref
                        .filtered_categories
                        .get(state_mut.panels.category_state.selected().unwrap_or(0))
                        .cloned();
                    state_mut.open_lens_menu(vec![path], ex);
                }
            }
            UblxAction::ListTop
            | UblxAction::ListBottom
            | UblxAction::MoveUp
            | UblxAction::MoveDown
            | UblxAction::MoveUpFast
            | UblxAction::MoveDownFast
            | UblxAction::FocusCategories
            | UblxAction::FocusContents
            | UblxAction::Tab => self.apply_navigation(state_mut, action),
            _ => {}
        }
        false
    }

    fn apply_right_pane(&self, state_mut: &mut UblxState, action: UblxAction) {
        match action {
            UblxAction::CycleRightPane => self.apply_cycle_right_pane(state_mut),
            UblxAction::RightPaneViewer => state_mut.right_pane_mode = RightPaneMode::Viewer,
            UblxAction::ViewerFullscreenToggle => {
                state_mut.chrome.viewer_fullscreen = !state_mut.chrome.viewer_fullscreen;
            }
            UblxAction::RightPaneTemplates if !self.right_content_ref.templates.is_empty() => {
                state_mut.right_pane_mode = RightPaneMode::Templates;
            }
            UblxAction::RightPaneMetadata if self.right_content_ref.metadata.is_some() => {
                state_mut.right_pane_mode = RightPaneMode::Metadata;
            }
            UblxAction::RightPaneWriting if self.right_content_ref.writing.is_some() => {
                state_mut.right_pane_mode = RightPaneMode::Writing;
            }
            _ => {}
        }
    }

    fn apply_navigation(&self, state_mut: &mut UblxState, action: UblxAction) {
        match action {
            UblxAction::ListTop => self.apply_list_top(state_mut),
            UblxAction::ListBottom => self.apply_list_bottom(state_mut),
            UblxAction::MoveUp => self.apply_move_up(state_mut),
            UblxAction::MoveDown => self.apply_move_down(state_mut),
            UblxAction::MoveUpFast => self.apply_move_up_by(state_mut, LIST_FAST_STEP_ROWS),
            UblxAction::MoveDownFast => self.apply_move_down_by(state_mut, LIST_FAST_STEP_ROWS),
            UblxAction::FocusCategories => {
                state_mut.panels.focus = PanelFocus::Categories;
                state_mut.multiselect.clear();
            }
            UblxAction::FocusContents => state_mut.panels.focus = PanelFocus::Contents,
            UblxAction::Tab => match state_mut.panels.focus {
                PanelFocus::Categories => state_mut.panels.focus = PanelFocus::Contents,
                PanelFocus::Contents => {
                    state_mut.multiselect.clear();
                    state_mut.panels.focus = PanelFocus::Categories;
                }
            },
            _ => {}
        }
    }

    fn apply_cycle_right_pane(&self, state_mut: &mut UblxState) {
        let available: Vec<RightPaneMode> = [
            RightPaneMode::Viewer,
            RightPaneMode::Templates,
            RightPaneMode::Metadata,
            RightPaneMode::Writing,
        ]
        .into_iter()
        .filter(|m| match m {
            RightPaneMode::Viewer => true,
            RightPaneMode::Templates => !self.right_content_ref.templates.is_empty(),
            RightPaneMode::Metadata => self.right_content_ref.metadata.is_some(),
            RightPaneMode::Writing => self.right_content_ref.writing.is_some(),
        })
        .collect();
        if !available.is_empty() {
            let idx = available
                .iter()
                .position(|m| *m == state_mut.right_pane_mode)
                .unwrap_or(0);
            let next = (idx + 1) % available.len();
            state_mut.right_pane_mode = available[next];
        }
    }

    fn apply_list_top(&self, state_mut: &mut UblxState) {
        match state_mut.panels.focus {
            PanelFocus::Categories => {
                if self.view_ref.category_list_len > 0 {
                    state_mut.panels.category_state.select(Some(0));
                }
            }
            PanelFocus::Contents => {
                if self.view_ref.content_len > 0 {
                    state_mut.panels.content_state.select(Some(0));
                }
            }
        }
    }

    fn apply_list_bottom(&self, state_mut: &mut UblxState) {
        match state_mut.panels.focus {
            PanelFocus::Categories => {
                if self.view_ref.category_list_len > 0 {
                    let last = clamp_selection(
                        self.view_ref.category_list_len,
                        self.view_ref.category_list_len,
                    );
                    state_mut.panels.category_state.select(Some(last));
                }
            }
            PanelFocus::Contents => {
                if self.view_ref.content_len > 0 {
                    let last =
                        clamp_selection(self.view_ref.content_len, self.view_ref.content_len);
                    state_mut.panels.content_state.select(Some(last));
                }
            }
        }
    }

    fn apply_move_up(&self, state_mut: &mut UblxState) {
        self.apply_move_up_by(state_mut, 1);
    }

    fn apply_move_up_by(&self, state_mut: &mut UblxState, step: usize) {
        match state_mut.panels.focus {
            PanelFocus::Categories => {
                if self.view_ref.category_list_len > 0 {
                    let i = state_mut.panels.category_state.selected().unwrap_or(0);
                    state_mut.panels.category_state.select(Some(clamp_selection(
                        i.saturating_sub(step),
                        self.view_ref.category_list_len,
                    )));
                }
            }
            PanelFocus::Contents => {
                if self.view_ref.content_len > 0 {
                    let i = state_mut.panels.content_state.selected().unwrap_or(0);
                    state_mut.panels.content_state.select(Some(clamp_selection(
                        i.saturating_sub(step),
                        self.view_ref.content_len,
                    )));
                }
            }
        }
    }

    fn apply_move_down(&self, state_mut: &mut UblxState) {
        self.apply_move_down_by(state_mut, 1);
    }

    fn apply_move_down_by(&self, state_mut: &mut UblxState, step: usize) {
        match state_mut.panels.focus {
            PanelFocus::Categories => {
                if self.view_ref.category_list_len > 0 {
                    let i = state_mut.panels.category_state.selected().unwrap_or(0);
                    state_mut.panels.category_state.select(Some(clamp_selection(
                        i.saturating_add(step),
                        self.view_ref.category_list_len,
                    )));
                }
            }
            PanelFocus::Contents => {
                if self.view_ref.content_len > 0 {
                    let i = state_mut.panels.content_state.selected().unwrap_or(0);
                    state_mut.panels.content_state.select(Some(clamp_selection(
                        i.saturating_add(step),
                        self.view_ref.content_len,
                    )));
                }
            }
        }
    }
}
