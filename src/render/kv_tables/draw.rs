//! Viewport math and drawing tables to the frame.

use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::layout::{setup::UblxState, style};
use crate::modules::viewer_search;
use crate::ui::UI_STRINGS;

use super::consts::TABLE_GAP;
use super::ratatui_table;
use super::sections;
use super::sections::{ContentsSection, KvSection, Section, SingleColumnListSection};

/// Visible line range for a section: (`skip_lines`, `take_lines`) or None if section is off-screen.
/// Uses `usize` so cumulative metadata line counts past 65535 do not wrap (which broke large tables).
#[derive(Clone, Copy)]
pub struct SectionRange {
    pub start: usize,
    pub height: usize,
}

#[derive(Clone, Copy)]
pub struct VisibleRange {
    pub start: usize,
    pub end: usize,
}

#[must_use]
pub fn visible_section_window(
    section: SectionRange,
    visible: VisibleRange,
) -> Option<(usize, usize)> {
    let section_end = section.start.saturating_add(section.height);
    if section_end <= visible.start || section.start >= visible.end {
        return None;
    }
    let seg_lo = section.start.max(visible.start);
    let seg_hi = section_end.min(visible.end);
    let take_lines = seg_hi.saturating_sub(seg_lo);
    if take_lines == 0 {
        return None;
    }
    let skip_lines = seg_lo.saturating_sub(section.start);
    Some((skip_lines, take_lines))
}

/// Rect for content at `y_offset` (from `table_area.y`) with height clamped so it doesn't exceed the viewport.
#[must_use]
pub fn rect_in_viewport(table_area: Rect, y_offset: usize, height: u16, viewport: u16) -> Rect {
    let y_off = y_offset.min(u16::MAX as usize) as u16;
    let max_h = viewport.saturating_sub(y_off);
    Rect {
        x: table_area.x,
        y: table_area.y.saturating_add(y_off),
        width: table_area.width,
        height: height.min(max_h),
    }
}

#[must_use]
fn current_find_line_idx(
    line_starts: Option<&[usize]>,
    find_ranges: &[(usize, usize)],
    find_current: usize,
) -> Option<usize> {
    let starts = line_starts?;
    let (start, _) = *find_ranges.get(find_current)?;
    let p = starts.partition_point(|&off| off <= start);
    Some(p.saturating_sub(1))
}

/// First data row index, row count, and Y offset for a ratatui table in the padded area.
///
/// `table_start_line` is the first line of the table widget (after any section title line).
/// `take_extra_cap` is used for Contents tables only: at most `viewport - 1` data rows so the
/// header row plus rows fit in the viewport.
struct TableRowWindow {
    skip: usize,
    take: usize,
    y_offset: usize,
}

impl TableRowWindow {
    /// Height in terminal rows: either data rows only, or one header row plus data rows (KV / Contents).
    #[must_use]
    fn rect_height(self, include_header_row: bool) -> u16 {
        let n = if include_header_row {
            1usize.saturating_add(self.take)
        } else {
            self.take
        };
        n.min(u16::MAX as usize) as u16
    }
}

/// Frame + padded table rect + vertical scroll window (shared by per-section draw helpers).
struct TableDrawCtx<'a, 'f> {
    f: &'a mut ratatui::Frame<'f>,
    table_area: Rect,
    viewport: u16,
    visible_start: usize,
    visible_end: usize,
    find_needle: Option<&'a str>,
    /// Haystack line starts when find has synced ranges (for `n`/`N` cell highlight alignment).
    line_starts: Option<&'a [usize]>,
    find_ranges: &'a [(usize, usize)],
    find_current: usize,
    /// Must match [`sections::parse_json_sections_with`] for Contents cell text.
    max_array_inline: usize,
    metadata_mode: bool,
    current_line_idx: Option<usize>,
}

impl TableDrawCtx<'_, '_> {
    /// Draw a section title line when it falls in the visible window.
    fn draw_section_title(&mut self, title: &str, section_start: usize, sub_title: bool) {
        if section_start < self.visible_start || section_start >= self.visible_end {
            return;
        }
        let dy = section_start.saturating_sub(self.visible_start);
        let ry = self
            .table_area
            .y
            .saturating_add(dy.min(u16::MAX as usize) as u16);
        if ry >= self.table_area.y.saturating_add(self.table_area.height) {
            return;
        }
        let title_style = if sub_title {
            style::table_section_subtitle_style()
        } else {
            style::table_section_title_style()
        };
        let title_upper = title.to_uppercase();
        let line = if let Some(line_starts) = self.line_starts {
            if self.find_ranges.is_empty() {
                ratatui::text::Line::from(title_upper).style(title_style)
            } else {
                let global_start = line_starts.get(section_start).copied().unwrap_or(0);
                let match_style = title_style.add_modifier(Modifier::UNDERLINED);
                let current_style = if self.metadata_mode {
                    style::viewer_find_match_current_metadata_contrast()
                } else {
                    match_style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                };
                viewer_search::highlight_line_with_find_styles(
                    &title_upper,
                    global_start,
                    self.find_ranges,
                    self.find_current,
                    title_style,
                    match_style,
                    current_style,
                )
            }
        } else {
            ratatui::text::Line::from(title_upper).style(title_style)
        };
        self.f.render_widget(
            ratatui::widgets::Paragraph::new(line),
            Rect {
                x: self.table_area.x,
                y: ry,
                width: self.table_area.width,
                height: 1,
            },
        );
    }

    /// Map scroll window to visible data rows. Returns `None` when nothing to draw.
    fn window_table_rows(
        &self,
        table_start_line: usize,
        take_lines: usize,
        num_rows: usize,
        take_extra_cap: Option<usize>,
    ) -> Option<TableRowWindow> {
        let skip = self
            .visible_start
            .saturating_sub(table_start_line)
            .min(num_rows);
        let mut take = take_lines.min(num_rows.saturating_sub(skip));
        if let Some(cap) = take_extra_cap {
            take = take.min(cap);
        }
        if take == 0 {
            return None;
        }
        Some(TableRowWindow {
            skip,
            take,
            y_offset: table_start_line.saturating_sub(self.visible_start),
        })
    }

    fn draw_kv_visible(
        &mut self,
        section_start: usize,
        take_lines: usize,
        has_title: bool,
        num_rows: usize,
        row_offset: usize,
        kv: &KvSection,
    ) {
        let table_start_line = section_start + usize::from(has_title);
        let Some(w) = self.window_table_rows(table_start_line, take_lines, num_rows, None) else {
            return;
        };
        let skip = w.skip;
        let take = w.take;
        let kv_visible = KvSection {
            title: None,
            rows: kv.rows[skip..skip + take].to_vec(),
            sub_title: false,
        };
        let rect = rect_in_viewport(
            self.table_area,
            w.y_offset,
            w.rect_height(true),
            self.viewport,
        );
        let first_data_line_idx = section_start
            .saturating_add(usize::from(has_title))
            .saturating_add(1);
        let find_kv_data = self.line_starts.and_then(|ls| {
            if self.find_ranges.is_empty() {
                return None;
            }
            Some(ratatui_table::KvFindSync {
                line_starts: ls,
                ranges: self.find_ranges,
                current: self.find_current,
                first_data_line_idx,
                row_skip: skip,
            })
        });
        self.f.render_widget(
            ratatui_table::section_to_table(
                &kv_visible,
                row_offset + skip,
                self.find_needle,
                find_kv_data.as_ref(),
                self.metadata_mode,
                rect.width.max(1),
            ),
            rect,
        );
    }

    fn draw_contents_visible(
        &mut self,
        section_start: usize,
        take_lines: usize,
        num_rows: usize,
        row_offset: usize,
        c: &ContentsSection,
    ) {
        let table_start_line = section_start + 1;
        let first_data_line_idx = table_start_line + 1;
        let take_cap = (self.viewport as usize).saturating_sub(1);
        let Some(w) =
            self.window_table_rows(table_start_line, take_lines, num_rows, Some(take_cap))
        else {
            return;
        };
        let skip = w.skip;
        let take = w.take;
        let rect = rect_in_viewport(
            self.table_area,
            w.y_offset,
            w.rect_height(true),
            self.viewport,
        );
        self.f.render_widget(
            ratatui_table::contents_to_table_window(
                c,
                ratatui_table::TableWindow {
                    row_offset: row_offset + skip,
                    start: skip,
                    end: skip + take,
                },
                rect.width,
                self.max_array_inline,
                ratatui_table::TableFindRenderCtx {
                    needle: self.find_needle,
                    current_line_idx: self.current_line_idx,
                    first_data_line_idx,
                    metadata_mode: self.metadata_mode,
                },
            ),
            rect,
        );
    }

    fn draw_single_column_list_visible(
        &mut self,
        section_start: usize,
        take_lines: usize,
        num_rows: usize,
        row_offset: usize,
        list: &SingleColumnListSection,
    ) {
        let table_start_line = section_start + 1;
        let first_data_line_idx = table_start_line;
        let Some(w) = self.window_table_rows(table_start_line, take_lines, num_rows, None) else {
            return;
        };
        let skip = w.skip;
        let take = w.take;
        let rect = rect_in_viewport(
            self.table_area,
            w.y_offset,
            w.rect_height(false),
            self.viewport,
        );
        self.f.render_widget(
            ratatui_table::single_column_list_to_table(
                list,
                ratatui_table::TableWindow {
                    row_offset,
                    start: skip,
                    end: skip + take,
                },
                ratatui_table::TableFindRenderCtx {
                    needle: self.find_needle,
                    current_line_idx: self.current_line_idx,
                    first_data_line_idx,
                    metadata_mode: self.metadata_mode,
                },
            ),
            rect,
        );
    }

    /// Walk parsed sections, advancing layout line counts and painting only the portion that intersects `visible`.
    fn draw_visible_table_sections(&mut self, sections: &[Section], visible: VisibleRange) {
        let mut line_index: usize = 0;
        let mut row_offset = 0;
        for (i, section) in sections.iter().enumerate() {
            if i > 0 {
                line_index += TABLE_GAP as usize;
            }
            let title_opt = section.title_str();
            let (_has_title, header_lines, num_rows) = section.line_metrics();
            let section_start = line_index;
            if title_opt.is_some() {
                line_index += 1;
            }
            line_index += header_lines as usize;
            line_index += num_rows;
            let section_height = line_index - section_start;

            let Some((_skip_lines, take_lines)) = visible_section_window(
                SectionRange {
                    start: section_start,
                    height: section_height,
                },
                visible,
            ) else {
                row_offset += num_rows;
                continue;
            };

            if let Some(title) = title_opt {
                self.draw_section_title(title, section_start, section.sub_title_style());
            }
            let has_title = title_opt.is_some();
            match section {
                Section::KeyValue(kv) => {
                    self.draw_kv_visible(
                        section_start,
                        take_lines,
                        has_title,
                        num_rows,
                        row_offset,
                        kv,
                    );
                }
                Section::Contents(c) => {
                    self.draw_contents_visible(section_start, take_lines, num_rows, row_offset, c);
                }
                Section::SingleColumnList(list) => {
                    self.draw_single_column_list_visible(
                        section_start,
                        take_lines,
                        num_rows,
                        row_offset,
                        list,
                    );
                }
                Section::Tree(tree) => {
                    let list = SingleColumnListSection {
                        title: tree.title.clone(),
                        values: super::schema::tree_roots_to_lines(&tree.roots),
                    };
                    self.draw_single_column_list_visible(
                        section_start,
                        take_lines,
                        num_rows,
                        row_offset,
                        &list,
                    );
                }
            }
            row_offset += num_rows;
        }
    }
}

/// Render Key/Value and Contents tables from JSON in the given rect, with scroll offset. Only the visible window is drawn; Contents rows are built only for visible range (memory optimization).
pub fn draw_tables(
    f: &mut ratatui::Frame,
    area: Rect,
    json: &str,
    scroll_y: u16,
    find_needle: Option<&str>,
    state: &UblxState,
) {
    use ratatui::widgets::Paragraph;

    let table_area = style::rect_with_h_pad(area);
    let ctx =
        super::KvParseCtx::from_table_width(table_area.width, state.panels.typed_column_tables);
    let sections = sections::parse_json_sections_with_ctx(json, ctx);
    if sections.is_empty() {
        f.render_widget(
            Paragraph::new(UI_STRINGS.pane.not_available).style(style::text_style()),
            area,
        );
        return;
    }
    let viewport = table_area.height;
    let visible_start = scroll_y as usize;
    let visible_end = visible_start.saturating_add(viewport as usize);
    let visible = VisibleRange {
        start: visible_start,
        end: visible_end,
    };
    let line_starts_vec = if viewer_search::option_needle_nonempty(find_needle)
        && !state.viewer_find.ranges.is_empty()
    {
        let hay = sections::searchable_text_from_json_with_ctx(json, ctx);
        Some(sections::line_byte_starts(&hay))
    } else {
        None
    };
    let line_starts = line_starts_vec.as_deref();
    let mut ctx = TableDrawCtx {
        f,
        table_area,
        viewport,
        visible_start,
        visible_end,
        find_needle,
        line_starts,
        find_ranges: &state.viewer_find.ranges,
        find_current: state.viewer_find.current,
        max_array_inline: ctx.max_array_inline,
        metadata_mode: state.right_pane_mode == crate::layout::setup::RightPaneMode::Metadata,
        current_line_idx: current_find_line_idx(
            line_starts,
            &state.viewer_find.ranges,
            state.viewer_find.current,
        ),
    };
    ctx.draw_visible_table_sections(&sections, visible);
}
