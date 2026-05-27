//! Right-pane viewers: CSV hints, markdown, comfy tables, image labels, zahir JSON helpers.

use std::path::Path;

use ublx::integrations::{
    ZahirFT, ZahirOutput, file_type_from_metadata_name, zahir_output_to_json_for_path,
};
use ublx::layout::setup::{RightPaneContent, RightPaneMode, UblxState, ViewerFindState};
use ublx::modules::viewer_search;
use ublx::render::{kv_tables, panes, viewers};
use ublx::ui::UI_GLYPHS;

// --- CSV --------------------------------------------------------------------

#[test]
fn path_hint_tsv_uses_tab() {
    let raw = "a\tb\nc\td\n";
    let rows = viewers::csv_handler::parse_csv(raw, Some("dir/file.tsv")).unwrap();
    assert_eq!(rows, vec![vec!["a", "b"], vec!["c", "d"]]);
}

#[test]
fn path_hint_psv_uses_pipe() {
    let raw = "a|b\nc|d\n";
    let rows = viewers::csv_handler::parse_csv(raw, Some("data.psv")).unwrap();
    assert_eq!(rows, vec![vec!["a", "b"], vec!["c", "d"]]);
}

#[test]
fn comma_without_hint_still_parses() {
    let raw = "a,b\n1,2\n";
    let rows = viewers::csv_handler::parse_csv(raw, None).unwrap();
    assert_eq!(rows, vec![vec!["a", "b"], vec!["1", "2"]]);
}

#[test]
fn csv_ragged_rows_still_parse_for_viewer_table() {
    let raw = "\"a\",\"b\"\n1,2\n3,4,5\n";
    let rows = viewers::csv_handler::parse_csv(raw, Some("data.csv")).unwrap();
    assert_eq!(
        rows,
        vec![vec!["a", "b"], vec!["1", "2"], vec!["3", "4", "5"],]
    );
}

#[test]
fn csv_wider_than_30_columns_falls_back_from_pretty_table() {
    let header = (0..31)
        .map(|i| format!("h{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let row = (0..31).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let raw = format!("{header}\n{row}\n");
    let rows = viewers::csv_handler::parse_csv(&raw, Some("wide.csv")).unwrap();
    assert!(!viewers::csv_handler::should_render_as_table(&rows));
}

#[test]
fn wide_csv_structured_view_shows_first_visible_columns() {
    let header = (0..40)
        .map(|i| format!("h{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let row = (0..40)
        .map(|i| format!("v{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let raw = format!("{header}\n{row}\n");
    let rows = viewers::csv_handler::parse_csv(&raw, Some("wide.csv")).unwrap();
    let s = viewers::csv_handler::wide_structured_string(&rows, 80, None);
    assert!(s.contains("[wide delimited view: truncated from 2 rows and 40 columns;"));
    assert!(s.contains("h0"));
    assert!(s.contains("v0"));
    assert!(
        !s.contains("h39"),
        "should not show far-right columns at width 80"
    );
}

#[test]
fn wide_csv_structured_view_uses_total_rows_hint_for_hidden_count() {
    let raw = "h0,h1\nv0,v1\n__UBLX_CSV_TOTAL_ROWS__=4096\n";
    let rows = viewers::csv_handler::parse_csv(raw, Some("wide.csv")).unwrap();
    assert_eq!(
        rows.len(),
        2,
        "meta trailer should be stripped from parsed rows"
    );
    let total = viewers::csv_handler::total_rows_hint_from_raw(raw);
    assert_eq!(total, Some(4096));
    let s = viewers::csv_handler::wide_structured_string(&rows, 80, total);
    assert!(s.contains("rows 2 shown, 4094 hidden"));
}

// --- Markdown ---------------------------------------------------------------

#[test]
fn paragraph_wraps_to_width() {
    let doc = viewers::markdown::parse_markdown("aa bb cc dd ee");
    let text = doc.to_text(8);
    assert!(
        text.lines.len() >= 2,
        "expected wrapped lines, got {}",
        text.lines.len()
    );
    for line in &text.lines {
        assert!(line.width() <= 8, "line {:?} exceeds width 8", line);
    }
}

#[test]
fn blockquote_wrap_respects_width() {
    let doc = viewers::markdown::parse_markdown(
        "> this is a long quoted line that must wrap when the viewport is narrow",
    );
    let text = doc.to_text(12);
    for line in &text.lines {
        assert!(line.width() <= 12, "quote line {:?} exceeds width 12", line);
    }
}

#[test]
fn list_item_wrap_respects_width() {
    let doc = viewers::markdown::parse_markdown(
        "- short prefix then many words here that should wrap across lines",
    );
    let w = 18u16;
    let text = doc.to_text(w);
    for line in &text.lines {
        assert!(
            line.width() <= w as usize,
            "list line {:?} exceeds width {}",
            line,
            w
        );
    }
}

fn assert_lines_within_width(doc: &viewers::markdown::MarkdownDoc, w: u16) {
    let text = doc.to_text(w);
    for line in &text.lines {
        assert!(
            line.width() <= w as usize,
            "line {:?} exceeds width {}",
            line,
            w
        );
    }
}

#[test]
fn fenced_code_block_parses_and_respects_width() {
    let doc = viewers::markdown::parse_markdown(
        r"```rust
fn main() {}
```",
    );
    let code = doc.blocks.iter().find_map(|b| match b {
        viewers::markdown::Block::Code { lang, text } => Some((lang.as_deref(), text.as_str())),
        _ => None,
    });
    assert!(code.is_some(), "expected Block::Code, got {:?}", doc.blocks);
    let (lang, text) = code.unwrap();
    assert_eq!(lang, Some("rust"));
    assert!(text.contains("main"));
    assert_lines_within_width(&doc, 40);
}

#[test]
fn gfm_table_parses_header_and_rows() {
    let md = r"| H1 | H2 |
| --- | --- |
| a | b |
";
    let doc = viewers::markdown::parse_markdown(md);
    let table = doc.blocks.iter().find_map(|b| match b {
        viewers::markdown::Block::Table { header, rows } => {
            Some((header.as_slice(), rows.as_slice()))
        }
        _ => None,
    });
    assert!(
        table.is_some(),
        "expected Block::Table, got {:?}",
        doc.blocks
    );
    let (header, rows) = table.unwrap();
    assert_eq!(header, &["H1", "H2"]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], vec!["a", "b"]);
}

#[test]
fn heading_block_has_level() {
    let doc = viewers::markdown::parse_markdown("# Title\n\nBody.");
    let heading = doc.blocks.iter().find_map(|b| match b {
        viewers::markdown::Block::Heading { level, .. } => Some(*level),
        _ => None,
    });
    assert_eq!(heading, Some(1));
}

#[test]
fn heading_to_text_respects_width() {
    let doc = viewers::markdown::parse_markdown("# Short\n");
    assert_lines_within_width(&doc, 20);
}

// --- Image viewer -----------------------------------------------------------

#[test]
fn label_body_error_includes_markdown_image_glyph() {
    let s = viewers::images::label_body_error("not found");
    assert!(s.contains("not found"));
    assert!(s.contains(UI_GLYPHS.markdown_image));
}

// --- Pretty tables (comfy-table grid + word wrap) --------------------------

/// Line count for cells joined with `\n`. `str::lines()` skips a final empty line after `\n`.
fn cell_visual_lines(s: &str) -> usize {
    s.split('\n').count()
}

#[test]
fn short_column_skips_wrap_but_pads() {
    let header = vec!["Long header that wraps".into(), "B".into()];
    let body = vec![vec![
        "first body line is long enough to wrap here yes".into(),
        "x".into(),
    ]];
    let (h, b) = viewers::pretty_tables::prepare_multiline_grid(&header, &body, 24);
    assert!(h[0].contains('\n'), "first col should wrap: {:?}", h);
    let header_h = cell_visual_lines(&h[0]);
    assert_eq!(
        cell_visual_lines(&h[1]),
        header_h,
        "short header cell is not word-wrapped but padded to row height: {:?}",
        h
    );
    assert_eq!(h[1].split('\n').next(), Some("B"));
    assert!(
        h[1].split('\n').skip(1).all(|l| l.is_empty()),
        "padding lines after short header text should be blank: {:?}",
        h[1]
    );
    let row0 = &b[0];
    assert!(row0[0].contains('\n'));
    let n0 = cell_visual_lines(&row0[0]);
    assert_eq!(
        cell_visual_lines(&row0[1]),
        n0,
        "short column padded to same visual height: {:?}",
        row0[1]
    );
    assert_eq!(row0[1].split('\n').next(), Some("x"));
}

#[test]
fn header_with_spaces_breaks_at_words_not_mid_word() {
    let header = vec!["Completion Date".into(), "B".into()];
    let body = vec![vec![
        "x".repeat(viewers::pretty_tables::VIEWER_TABLE_NO_WRAP_COL_MAX_CHARS + 1),
        "y".into(),
    ]];
    let (h, _) = viewers::pretty_tables::prepare_multiline_grid(&header, &body, 24);
    let lines: Vec<&str> = h[0].split('\n').collect();
    assert!(
        lines.contains(&"Completion"),
        "expected whole word line, got {:?}",
        lines
    );
    assert!(
        lines.contains(&"Date"),
        "expected whole word line, got {:?}",
        lines
    );
    assert!(
        !lines.iter().any(|l| *l == "Comp" || *l == "leti"),
        "should not hard-break inside Completion: {:?}",
        lines
    );
}

#[test]
fn word_wrap_breaks_long_word() {
    let lines = viewers::pretty_tables::word_wrap_text("abcdefghij", 4);
    assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
}

#[test]
fn word_wrap_spaces() {
    let lines = viewers::pretty_tables::word_wrap_text("aa bb cc", 5);
    assert_eq!(lines, vec!["aa bb", "cc"]);
}

#[test]
fn word_wrap_cell_splits_isodate_on_hyphens_not_mid_digit() {
    let lines = viewers::pretty_tables::word_wrap_cell_text("2025-10-25", 4);
    assert_eq!(lines, vec!["2025", "-10", "-25"]);
}

#[test]
fn word_wrap_cell_hyphen_long_word_fallback() {
    let lines = viewers::pretty_tables::word_wrap_cell_text("abcdefghij", 4);
    assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
}

// --- Zahir wrapper ----------------------------------------------------------

#[test]
fn wrapper_matches_zahirscan_api() {
    assert_eq!(file_type_from_metadata_name("CSV"), Some(ZahirFT::Csv));
    assert_eq!(
        file_type_from_metadata_name("Markdown"),
        Some(ZahirFT::Markdown)
    );
}

#[test]
fn non_zahir_categories_miss() {
    assert_eq!(file_type_from_metadata_name("Directory"), None);
    assert_eq!(file_type_from_metadata_name("not a label"), None);
}

#[test]
fn zahir_json_for_path_injects_file_type_when_empty() {
    let o = ZahirOutput::default();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let full = root.join("src/lib.rs");
    let j = zahir_output_to_json_for_path(Some(&o), &full, "src/lib.rs");
    assert!(
        j.contains(r#""file_type":"Code""#),
        "expected path-based Code for .rs, got {j}"
    );
}

#[test]
fn viewer_total_lines_kv_tables_matches_content_height() {
    let json = r#"{"field": "value"}"#;
    let mut state = UblxState::new();
    state.right_pane_mode = RightPaneMode::Metadata;
    let mut rc = RightPaneContent::empty();
    rc.metadata = Some(json.to_string());
    let w = 80u16;
    let n = panes::viewer_total_lines(&rc, w, Some(json), &mut state, None);
    assert_eq!(n, kv_tables::content_height(json, ublx::config::ColumnStatsDisplay::default()) as usize);
}

#[test]
fn literal_match_ranges_empty_needle() {
    assert!(viewer_search::literal_match_ranges("hello world", "").is_empty());
    assert!(viewer_search::literal_match_ranges("hello world", "   ").is_empty());
}

#[test]
fn literal_match_ranges_no_matches() {
    assert!(viewer_search::literal_match_ranges("abc", "z").is_empty());
}

#[test]
fn literal_match_ranges_multiple() {
    assert_eq!(
        viewer_search::literal_match_ranges("abab", "ab"),
        vec![(0, 2), (2, 4)]
    );
}

#[test]
fn line_byte_to_index_empty_haystack() {
    assert_eq!(viewer_search::line_byte_to_index("", 0), 0);
    assert_eq!(viewer_search::line_byte_to_index("", 5), 0);
}

#[test]
fn line_byte_to_index_newlines() {
    let s = "a\nb\nc";
    assert_eq!(viewer_search::line_byte_to_index(s, 0), 0);
    assert_eq!(viewer_search::line_byte_to_index(s, 2), 1);
    assert_eq!(viewer_search::line_byte_to_index(s, 4), 2);
}

#[test]
fn scroll_preview_to_current_first_last_viewport_1() {
    let hay = "line0\nline1\nline2\nline3";
    let mut state = UblxState::new();
    state.viewer_find = ViewerFindState {
        ranges: vec![(0, 4)], // "line" at start of line0
        current: 0,
        ..Default::default()
    };
    viewer_search::scroll_preview_to_current(&mut state, hay, 1);
    assert_eq!(state.panels.preview_scroll, 0);

    state.viewer_find.ranges = vec![(hay.len().saturating_sub(5), hay.len())];
    state.viewer_find.current = 0;
    viewer_search::scroll_preview_to_current(&mut state, hay, 1);
    // last line index 3, v/2 = 0 -> scroll 3
    assert_eq!(state.panels.preview_scroll, 3);
}

#[test]
fn scroll_preview_to_current_viewport_centers() {
    let hay = (0..10)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let line5_start = hay
        .match_indices('\n')
        .nth(4)
        .map(|(i, _)| i + 1)
        .unwrap_or(0);
    let mut state = UblxState::new();
    state.viewer_find.ranges = vec![(line5_start, line5_start + 4)];
    state.viewer_find.current = 0;
    viewer_search::scroll_preview_to_current(&mut state, &hay, 4);
    let line = viewer_search::line_byte_to_index(&hay, line5_start);
    let expected = line.saturating_sub(4 / 2);
    assert_eq!(state.panels.preview_scroll, expected);
}

#[test]
fn highlight_table_cell_line_match_inside_line() {
    let line = viewer_search::highlight_table_cell_line("foo bar baz", 10, &[(13, 16)], 0);
    assert!(line.spans.len() > 1, "expected highlighted segment split");
}

#[test]
fn highlight_cell_line_two_occurrences() {
    let line = viewer_search::highlight_cell_line("a x a x", "x");
    assert!(line.spans.len() >= 3);
}

#[test]
fn highlighted_body_empty_ranges_is_raw() {
    let mut state = UblxState::new();
    state.viewer_find.ranges = vec![];
    let t = viewer_search::highlighted_body(&state, "one\ntwo");
    assert_eq!(ublx::render::panes::text_to_plain_string(&t), "one\ntwo");
}

#[test]
fn viewer_find_state_predicates() {
    let mut vf = ViewerFindState::default();
    assert!(!vf.needle_nonempty());
    assert!(!vf.find_affects_view());
    assert!(!vf.title_bottom_visible());

    vf.query = "  hi  ".to_string();
    assert!(vf.needle_nonempty());
    assert!(vf.title_bottom_visible());
    assert!(!vf.find_affects_view());

    vf.active = true;
    assert!(vf.find_affects_view());
}

#[test]
fn option_needle_nonempty_trims() {
    assert!(!viewer_search::option_needle_nonempty(None));
    assert!(!viewer_search::option_needle_nonempty(Some("")));
    assert!(!viewer_search::option_needle_nonempty(Some("  ")));
    assert!(viewer_search::option_needle_nonempty(Some(" x ")));
}
