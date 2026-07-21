//! Help overlay: keybinding tables. Sections depend on the active main tab; the main-tab digit row
//! matches visible tabs ([`crate::ui::main_tab_keys_help_keys_line`]).

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};

use crate::layout::{setup::MainMode, style};
use crate::themes;
use crate::ui::{
    CTRL_MENU_ROWS, UI_CONSTANTS, UI_GLYPHS, UI_STRINGS, main_tab_keys_help_keys_line,
};

macro_rules! help_entries {
    ($( ($key:expr, $action:expr) ),* $(,)?) => {
        &[ $( ($key, $action) ),* ]
    };
}

const WIDTH_LIMIT: usize = 24;
const DESC_MIN_WIDTH: usize = WIDTH_LIMIT - 4;

/// General: navigation, motion, sort, pane focus, Ctrl+j/k, quit/help.
const HELP_GENERAL_BROWSER: &[(&str, &str)] = help_entries![
    ("~", "Alternate between Main tabs"),
    (
        "/",
        "Fuzzy catalog filter; Enter (apply) · / (re-edit) · Esc (clear)"
    ),
    (
        "Tab",
        "Switch left or middle pane focus (categories ↔ contents)"
    ),
    ("h | l", "Focus on Left or Middle panes"),
    ("j | k", "Move down / up in Left or Middle panes"),
    (
        "gg | G",
        "Go to top / bottom of list (Left or Middle panes)"
    ),
    ("s", "Cycle sort mode (middle pane)"),
    (
        "Ctrl+j/k | Ctrl+↑↓",
        "Jump down / up by 10 in Left or Middle panes"
    ),
    (
        "q | Esc",
        "Quit (Esc also clears search / find when active)"
    ),
    ("?", "Toggle this help"),
];

/// Viewer pane: right-pane tab keys, Shift shortcuts (preview scroll, search, fullscreen).
const HELP_VIEWER: &[(&str, &str)] = help_entries![
    (
        "v/t/m/w",
        "Focus on Viewer/Templates/Metadata/Writing tab in right pane"
    ),
    ("Shift+Tab", "Cycle right pane tab(s)"),
    ("Shift+↓↑ | Shift+J/+K", "Scroll down / up in right pane"),
    ("Shift+b | Shift+e", "Jump right pane to top / bottom"),
    (
        "Shift+S",
        "Viewer literal search; Enter (apply) · Shift+S (re-edit) · n/N (next/prev) · Esc (clear)"
    ),
    ("Shift+F", "Toggle fullscreen"),
];

/// Multi-select: contents pane, Snapshot or Lenses (not Duplicates). Bulk menu rows: **a** Add · **r** Rename · **d** Delete.
const HELP_MULTISELECT: &[(&str, &str)] = help_entries![
    ("Spacebar", "Toggle row for multi-select"),
    ("a", "Open Bulk menu"),
    ("Bulk menu → a", "Add to Lens/other Lens"),
    ("Bulk menu → r", "Rename paths in $EDITOR"),
    ("Bulk menu → d", "Delete files/remove from current Lens"),
    ("Bulk menu → z", "Enhance with ZahirScan"),
    ("Esc", "Exit Multi-select mode"),
];

/// Duplicates tab: no multi-select; Space is the small Duplicates menu (not full Actions).
const HELP_BROWSER_QA_DUPLICATES: &[(&str, &str)] = help_entries![
    ("d", "Delete file; duplicate list reloads from index"),
    (
        "i",
        "Ignore — hide path in Duplicates for current dupe-finder run"
    ),
];

/// quick actions menu (spacebar): matches [`crate::ui::qa_menu_item_labels`] / `UI_STRINGS.space`.
const HELP_BROWSER_QA: &[(&str, &str)] = help_entries![
    ("o", "Open — Terminal and/or GUI"),
    ("f", "Show in folder"),
    ("p", "Enhance policy"),
    ("z", "Enhance with ZahirScan"),
    ("l", "Add to Lens"),
    ("c", "Copy Path"),
    ("j", "Copy Zahir JSON"),
    ("r", "Rename file or lens"),
    ("d", "Delete file; remove from lens; delete lens"),
];

/// Settings tab: one General table (digit row + nav + other + closing).
const HELP_SETTINGS_GENERAL: &[(&str, &str)] = help_entries![
    ("Tab", "Switch Global vs Local config scope"),
    ("j | k", "Move up / down in the focused list"),
    ("e", "Open active Settings file in $EDITOR"),
    (
        "q | Esc",
        "Quit (Esc also clears search / find when active)"
    ),
    ("?", "Toggle this help"),
];

/// Second column for the main-tab digit row; same on every tab.
const MAIN_TAB_DIGITS_DESC: &str = "Jump to Main Tab number based on what is visible.";

/// Shown below all help tables: shortcuts and menu rows only appear when they apply.
const HELP_AVAILABILITY_FOOTNOTE: &str =
    "Tab, pane, current highlight, and config gate what is seen/available.";

#[derive(Clone, Copy)]
struct HelpSectionSpec {
    title: &'static str,
    rows: &'static [(&'static str, &'static str)],
    include_digit_row: bool,
}

/// General + Right Pane + Quick Actions — only `qa_rows` varies by tab.
const fn help_prefix_gen_view_qa(
    qa_rows: &'static [(&'static str, &'static str)],
) -> [HelpSectionSpec; 3] {
    [
        HelpSectionSpec {
            title: UI_STRINGS.tables.first_title,
            rows: HELP_GENERAL_BROWSER,
            include_digit_row: true,
        },
        HelpSectionSpec {
            title: UI_STRINGS.dialogs.help_section_viewer,
            rows: HELP_VIEWER,
            include_digit_row: false,
        },
        HelpSectionSpec {
            title: UI_STRINGS.dialogs.help_section_qa,
            rows: qa_rows,
            include_digit_row: false,
        },
    ]
}

const HELP_SECTION_MULTISELECT: HelpSectionSpec = HelpSectionSpec {
    title: UI_STRINGS.dialogs.multiselect_help_title,
    rows: HELP_MULTISELECT,
    include_digit_row: false,
};

const HELP_SECTION_COMMAND: HelpSectionSpec = HelpSectionSpec {
    title: UI_STRINGS.dialogs.command_mode_popup,
    rows: CTRL_MENU_ROWS,
    include_digit_row: false,
};

const BROWSER_PREFIX: [HelpSectionSpec; 3] = help_prefix_gen_view_qa(HELP_BROWSER_QA);

const BROWSER_SECTIONS: &[HelpSectionSpec] = &[
    BROWSER_PREFIX[0],
    BROWSER_PREFIX[1],
    BROWSER_PREFIX[2],
    HELP_SECTION_MULTISELECT,
    HELP_SECTION_COMMAND,
];

const DUPLICATES_PREFIX: [HelpSectionSpec; 3] = help_prefix_gen_view_qa(HELP_BROWSER_QA_DUPLICATES);

const DUPLICATES_SECTIONS: &[HelpSectionSpec] = &[
    DUPLICATES_PREFIX[0],
    DUPLICATES_PREFIX[1],
    DUPLICATES_PREFIX[2],
    HELP_SECTION_COMMAND,
];

const LENSES_PREFIX: [HelpSectionSpec; 3] = help_prefix_gen_view_qa(HELP_BROWSER_QA);

const LENSES_SECTIONS: &[HelpSectionSpec] = &[
    LENSES_PREFIX[0],
    LENSES_PREFIX[1],
    LENSES_PREFIX[2],
    HELP_SECTION_MULTISELECT,
    HELP_SECTION_COMMAND,
];

const SETTINGS_SECTIONS: &[HelpSectionSpec] = &[HelpSectionSpec {
    title: UI_STRINGS.tables.first_title,
    rows: HELP_SETTINGS_GENERAL,
    include_digit_row: true,
}];

/// Delta tab: same General navigation table as Snapshot; no viewer / multi-select / space rows (different layout).
const DELTA_SECTIONS: &[HelpSectionSpec] = &[HelpSectionSpec {
    title: UI_STRINGS.tables.first_title,
    rows: HELP_GENERAL_BROWSER,
    include_digit_row: true,
}];

#[must_use]
fn help_sections(mode: MainMode) -> &'static [HelpSectionSpec] {
    match mode {
        MainMode::Snapshot => BROWSER_SECTIONS,
        MainMode::Settings => SETTINGS_SECTIONS,
        MainMode::Delta => DELTA_SECTIONS,
        MainMode::Lenses => LENSES_SECTIONS,
        MainMode::Duplicates => DUPLICATES_SECTIONS,
    }
}

#[must_use]
fn main_tab_label(mode: MainMode) -> &'static str {
    match mode {
        MainMode::Snapshot => UI_STRINGS.main_tabs.snapshot,
        MainMode::Delta => UI_STRINGS.main_tabs.delta,
        MainMode::Settings => UI_STRINGS.main_tabs.settings,
        MainMode::Duplicates => UI_STRINGS.main_tabs.duplicates,
        MainMode::Lenses => UI_STRINGS.main_tabs.lenses,
    }
}

#[must_use]
fn help_context_blurb(mode: MainMode) -> String {
    let prefix = "Current Mode: ";
    let viewer_tabs = "right pane file viewer.";
    let body = match mode {
        MainMode::Snapshot => {
            format!("category tree, file list, and {viewer_tabs}")
        }
        MainMode::Delta => "snapshot overview and added / modified / removed lists.".to_string(),
        MainMode::Lenses => {
            format!("lens names, paths in the selected lens, and {viewer_tabs}")
        }
        MainMode::Duplicates => {
            format!("duplicate groups, member paths, and {viewer_tabs}")
        }
        MainMode::Settings => "edit Global or Local settings.".to_string(),
    };
    let tab_command_note = "Tab to switch help sections.";
    format!("{prefix}{body}\n\n{tab_command_note}")
}

/// Word-wrap line count for sizing the blurb (respects `\n`; each segment is wrapped by width).
fn wrap_line_count(text: &str, max_width: usize) -> usize {
    if max_width < 8 {
        return text.split('\n').count().max(1);
    }
    let mut total = 0usize;
    for segment in text.split('\n') {
        total += wrap_line_count_words(segment, max_width);
    }
    total.max(1)
}

fn wrap_line_count_words(text: &str, max_width: usize) -> usize {
    let mut lines = 0usize;
    let mut line_len = 0usize;
    for word in text.split_whitespace() {
        let w = word.chars().count();
        if line_len == 0 {
            line_len = w;
        } else if line_len + 1 + w <= max_width {
            line_len += 1 + w;
        } else {
            lines += 1;
            line_len = w;
        }
    }
    if line_len > 0 {
        lines += 1;
    }
    if lines == 0 && text.trim().is_empty() {
        return 1;
    }
    lines.max(1)
}

fn help_max_key_width(sections: &[HelpSectionSpec], main_keys: &str) -> u16 {
    let mut m = main_keys.len();
    for s in sections {
        for (k, _) in s.rows {
            m = m.max(k.len());
        }
        if s.include_digit_row {
            m = m.max(main_keys.len());
        }
    }
    m = m.max(UI_STRINGS.dialogs.help_command.len());
    u16::try_from(m.min(WIDTH_LIMIT)).unwrap_or(0)
}

fn help_max_desc_width(sections: &[HelpSectionSpec]) -> usize {
    let mut m = MAIN_TAB_DIGITS_DESC.len();
    for s in sections {
        for (_, d) in s.rows {
            m = m.max(d.len());
        }
    }
    m
}

/// Number of help section tabs for the current main mode (drives node strip + [`Tab`] cycling).
#[must_use]
pub fn help_tab_count(mode: MainMode) -> usize {
    help_sections(mode).len()
}

/// Section title (1) + table (header + data rows): `2 + dr`, `dr` = digit row + data rows.
fn section_table_height(s: &HelpSectionSpec) -> u16 {
    let dr = s.rows.len() + usize::from(s.include_digit_row);
    u16::try_from(2usize.saturating_add(dr)).unwrap_or(u16::MAX)
}

/// Fixed body area height so the popup does not resize when switching tabs.
fn max_section_table_block_height(sections: &[HelpSectionSpec]) -> u16 {
    let mut m = 3u16;
    for s in sections {
        m = m.max(section_table_height(s));
    }
    m
}

fn help_tab_node_line(
    sections: &[HelpSectionSpec],
    active: usize,
    popup_bg: Color,
    command_mode_leader: char,
) -> Line<'static> {
    let gap_style = Style::default().bg(popup_bg);
    let gap_n = usize::from(UI_CONSTANTS.main_tab_node_gap_cells);
    let mut segments: Vec<Span<'static>> = Vec::new();
    for (i, s) in sections.iter().enumerate() {
        if i > 0 {
            for _ in 0..gap_n {
                segments.push(Span::styled(" ", gap_style));
            }
        }
        let title = section_display_title(s, command_mode_leader);
        segments.extend(style::tab_node_segment(&title, i == active, false));
    }
    Line::from(segments)
}

fn section_display_title(s: &HelpSectionSpec, command_mode_leader: char) -> String {
    if s.title == UI_STRINGS.dialogs.command_mode_popup {
        crate::config::command_mode_popup_title(command_mode_leader)
    } else {
        s.title.to_string()
    }
}

fn build_help_table(
    rows: &[(&'static str, &'static str)],
    include_digit: bool,
    main_keys: &str,
    key_width: u16,
    text_style: Style,
) -> Table<'static> {
    let header = Row::new(vec![
        UI_STRINGS.dialogs.help_command,
        UI_STRINGS.dialogs.help_action,
    ])
    .style(style::table_header_style())
    .bottom_margin(0);

    let mut data_rows: Vec<Row> = Vec::new();
    let mut i = 0usize;
    if include_digit {
        data_rows.push(
            Row::new(vec![
                Cell::from(main_keys.to_string()),
                Cell::from(MAIN_TAB_DIGITS_DESC),
            ])
            .style(style::table_row_style(i)),
        );
        i += 1;
    }
    for (k, d) in rows {
        data_rows
            .push(Row::new(vec![Cell::from(*k), Cell::from(*d)]).style(style::table_row_style(i)));
        i += 1;
    }

    Table::new(
        data_rows,
        [
            Constraint::Length(key_width),
            Constraint::Min(u16::try_from(DESC_MIN_WIDTH).unwrap_or(0)),
        ],
    )
    .header(header)
    .column_spacing(1)
    .style(text_style)
}

struct HelpPopupLayout {
    popup_rect: Rect,
    block: Block<'static>,
    chunks: Vec<Rect>,
    main_keys: String,
    tab_blurb_indented: String,
    footnote_indented: String,
    sections: &'static [HelpSectionSpec],
    active_tab: usize,
    key_width: u16,
    text_style: Style,
    max_table_block_h: u16,
    popup_bg: Color,
    command_mode_leader: char,
}

fn compute_help_popup_layout(
    area: Rect,
    main_mode: MainMode,
    has_lenses: bool,
    has_duplicates: bool,
    help_tab: &mut u8,
    command_mode_leader: char,
) -> HelpPopupLayout {
    let sections = help_sections(main_mode);
    let n = sections.len().max(1);
    *help_tab = (*help_tab).min(n.saturating_sub(1) as u8);
    let active_tab = *help_tab as usize;

    let main_keys = main_tab_keys_help_keys_line(has_lenses, has_duplicates);
    let tab_blurb = help_context_blurb(main_mode);

    let key_width = help_max_key_width(sections, &main_keys);
    let desc_max = help_max_desc_width(sections);
    let content_w = (key_width as usize + 1 + desc_max).max(48);
    let tab_blurb_indented = tab_blurb;
    let wrap_w = content_w.saturating_sub(6).max(32);
    let blurb_lines = wrap_line_count(&tab_blurb_indented, wrap_w);
    let blurb_h = u16::try_from(blurb_lines).unwrap_or(u16::MAX).max(1);

    let footnote_indented = HELP_AVAILABILITY_FOOTNOTE.trim().to_string();
    let footnote_lines = wrap_line_count(&footnote_indented, wrap_w).max(1);
    let footnote_h = u16::try_from(footnote_lines).unwrap_or(u16::MAX).max(1);

    let max_table_block_h = max_section_table_block_height(sections);
    // Current mode blurb → gap → help section tabs (nodes) → gap → table body → footnote + GitHub.
    let inner_h = usize::from(blurb_h + 1 + 1 + 1 + max_table_block_h + 1 + footnote_h + 1 + 1);
    let content_h = inner_h + 1;

    let rect = style::centered_popup_rect(
        area,
        content_w,
        content_h,
        UI_CONSTANTS.popup_padding_w,
        UI_CONSTANTS.popup_padding_h,
    );

    let t = themes::current();
    let tab = main_tab_label(main_mode);
    let title_line = Line::from(vec![
        Span::raw(UI_CONSTANTS.empty_space),
        Span::styled(UI_STRINGS.dialogs.help, Style::default().fg(t.text)),
        Span::raw(" — "),
        Span::styled(tab, Style::default().fg(t.hint)),
        Span::raw(UI_CONSTANTS.empty_space),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title_line.centered())
        .border_style(Style::default().fg(t.focused_border))
        .style(Style::default().bg(t.popup_bg));
    let text_style = Style::default().fg(t.text).bg(t.popup_bg);
    let popup_bg = t.popup_bg;

    let inner = block.inner(rect);

    let constraints = vec![
        Constraint::Length(blurb_h),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(max_table_block_h),
        Constraint::Length(1),
        Constraint::Length(footnote_h),
        Constraint::Length(1),
        Constraint::Length(1),
    ];

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner)
        .to_vec();

    HelpPopupLayout {
        popup_rect: rect,
        block,
        chunks,
        main_keys,
        tab_blurb_indented,
        footnote_indented,
        sections,
        active_tab,
        key_width,
        text_style,
        max_table_block_h,
        popup_bg,
        command_mode_leader,
    }
}

/// Screen rect for the bottom “GitHub” row in the help overlay (for mouse hit-testing).
#[must_use]
pub fn help_github_footer_rect(
    frame_area: Rect,
    main_mode: MainMode,
    has_lenses: bool,
    has_duplicates: bool,
    help_tab: u8,
    command_mode_leader: char,
) -> Rect {
    let mut t = help_tab;
    let layout = compute_help_popup_layout(
        frame_area,
        main_mode,
        has_lenses,
        has_duplicates,
        &mut t,
        command_mode_leader,
    );
    layout.chunks.last().copied().unwrap_or(Rect::default())
}

pub fn render_help_box(
    f: &mut Frame,
    main_mode: MainMode,
    has_lenses: bool,
    has_duplicates: bool,
    help_tab: &mut u8,
    command_mode_leader: char,
) {
    let layout = compute_help_popup_layout(
        f.area(),
        main_mode,
        has_lenses,
        has_duplicates,
        help_tab,
        command_mode_leader,
    );
    let chunks = &layout.chunks;
    debug_assert_eq!(chunks.len(), 9);

    f.render_widget(Clear, layout.popup_rect);

    let blurb_para = Paragraph::new(layout.tab_blurb_indented.clone())
        .style(layout.text_style)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
    let gap = Paragraph::new("").style(layout.text_style);

    f.render_widget(layout.block, layout.popup_rect);

    f.render_widget(blurb_para, chunks[0]);
    f.render_widget(gap.clone(), chunks[1]);

    let tab_line = help_tab_node_line(
        layout.sections,
        layout.active_tab,
        layout.popup_bg,
        layout.command_mode_leader,
    );
    f.render_widget(
        Paragraph::new(tab_line)
            .alignment(Alignment::Center)
            .style(layout.text_style),
        chunks[2],
    );
    f.render_widget(gap.clone(), chunks[3]);

    let body = chunks[4];
    let s = &layout.sections[layout.active_tab];
    let section_title = section_display_title(s, layout.command_mode_leader);
    let inner_body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(layout.max_table_block_h.saturating_sub(1)),
        ])
        .split(body);
    let title_para = Paragraph::new(Line::from(vec![Span::styled(
        section_title,
        style::table_section_title_style().add_modifier(Modifier::UNDERLINED),
    )]))
    .alignment(Alignment::Center)
    .style(layout.text_style);
    f.render_widget(title_para, inner_body[0]);

    let table = build_help_table(
        s.rows,
        s.include_digit_row,
        &layout.main_keys,
        layout.key_width,
        layout.text_style,
    );
    let table_rect = style::rect_with_h_pad(inner_body[1]);
    f.render_widget(table, table_rect);

    let t = themes::current();
    let pre_footnote_gap = Paragraph::new("").style(layout.text_style);
    f.render_widget(pre_footnote_gap, chunks[5]);

    let hint = Style::default().fg(t.hint).bg(t.popup_bg);
    let footnote_para = Paragraph::new(layout.footnote_indented.clone())
        .style(hint)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
    f.render_widget(footnote_para, chunks[6]);

    let github_line = Line::from(vec![
        Span::styled(format!("{} ", UI_GLYPHS.github_mark), layout.text_style),
        Span::styled(
            "Repo",
            Style::default()
                .fg(t.text)
                .bg(t.popup_bg)
                .add_modifier(Modifier::UNDERLINED)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let footer_gap = Paragraph::new("").style(layout.text_style);
    let footer_para = Paragraph::new(github_line)
        .alignment(Alignment::Center)
        .style(layout.text_style);
    f.render_widget(footer_gap, chunks[7]);
    f.render_widget(footer_para, chunks[8]);
}
