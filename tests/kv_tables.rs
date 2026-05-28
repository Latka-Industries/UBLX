//! JSON sections, viewport math, column widths for metadata/writing tables.

use ratatui::layout::Rect;

use serde_json::{Value, json};
use ublx::config::PARALLEL;
use ublx::render::kv_tables::{
    SectionRange, VisibleRange, content_height,
    format::DEFAULT_MAX_ARRAY_INLINE,
    line_byte_starts, parse_json_sections,
    ratatui_table::{balanced_column_widths, contents_natural_widths},
    rect_in_viewport, searchable_text_from_json, visible_section_window,
};

#[test]
fn parse_json_kv_only() {
    let sections = parse_json_sections(r#"{"alpha": 1, "beta": "two"}"#);
    assert_eq!(sections.len(), 1);
    match &sections[0] {
        ublx::render::kv_tables::Section::KeyValue(kv) => {
            assert!(kv.title.as_ref().is_some_and(|t| !t.is_empty()));
            assert_eq!(kv.rows.len(), 2);
        }
        _ => panic!("expected KeyValue"),
    }
}

#[test]
fn parse_json_contents_entries() {
    let json = r#"{"meta": 1, "entries": [{"col_a": "x", "col_b": 2}]}"#;
    let sections = parse_json_sections(json);
    assert!(
        sections.len() >= 2,
        "expected KV + Contents, got {}",
        sections.len()
    );
    let has_contents = sections
        .iter()
        .any(|s| matches!(s, ublx::render::kv_tables::Section::Contents(_)));
    assert!(has_contents);
}

#[test]
fn parse_json_empty_object() {
    let sections = parse_json_sections("{}");
    assert!(sections.is_empty());
}

#[test]
fn searchable_text_stable_twice() {
    let json = r#"{"k": "v"}"#;
    let a = searchable_text_from_json(json);
    let b = searchable_text_from_json(json);
    assert_eq!(a, b);
}

#[test]
fn line_byte_starts_matches_joined_lines() {
    let joined = ["a", "bb", "ccc"].join("\n");
    let starts = line_byte_starts(&joined);
    assert_eq!(starts, vec![0, 2, 5]);
}

#[test]
fn content_height_matches_kv_metrics() {
    let json = r#"{"x": 1}"#;
    let h = content_height(json, ublx::config::ColumnStatsDisplay::default());
    let sections = parse_json_sections(json);
    let mut expected: u16 = 0;
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            expected += ublx::render::kv_tables::consts::TABLE_GAP;
        }
        let (has_title, header_lines, num_rows) = section.line_metrics();
        expected += u16::from(has_title);
        expected += header_lines;
        expected += num_rows as u16;
    }
    assert_eq!(h, expected);
}

#[test]
fn visible_section_window_fully_above() {
    assert!(
        visible_section_window(
            SectionRange {
                start: 0,
                height: 5
            },
            VisibleRange { start: 10, end: 20 }
        )
        .is_none()
    );
}

#[test]
fn visible_section_window_fully_below() {
    assert!(
        visible_section_window(
            SectionRange {
                start: 30,
                height: 5
            },
            VisibleRange { start: 0, end: 10 }
        )
        .is_none()
    );
}

#[test]
fn visible_section_window_partial_overlap() {
    assert_eq!(
        visible_section_window(
            SectionRange {
                start: 0,
                height: 10
            },
            VisibleRange { start: 0, end: 5 }
        ),
        Some((0, 5))
    );
}

#[test]
fn rect_in_viewport_clamps_height() {
    let r = rect_in_viewport(
        Rect {
            x: 2,
            y: 3,
            width: 40,
            height: 100,
        },
        5,
        20,
        10,
    );
    assert_eq!(r.height, 5);
    assert_eq!(r.y, 8);
}

#[test]
fn balanced_column_widths_distributes_remainder() {
    let w = balanced_column_widths(&[10, 10], 25, 1);
    assert_eq!(w.len(), 2);
    let sum: usize = w.iter().map(|&x| x as usize).sum();
    // 25 total width − 1 column gap = 24 for cells
    assert_eq!(sum, 24);
}

#[test]
fn contents_natural_widths_serial_small_window() {
    let section = ublx::render::kv_tables::ContentsSection {
        title: "T".to_string(),
        columns: vec!["A".to_string(), "B".to_string()],
        column_keys: vec!["a".to_string(), "b".to_string()],
        entries: vec![
            json!({"a": "short", "b": "loooooong"}),
            json!({"a": "wide_header", "b": "x"}),
        ],
        sub_title: false,
    };
    let n = contents_natural_widths(&section, 0, 2, DEFAULT_MAX_ARRAY_INLINE);
    assert_eq!(n.len(), 2);
    assert!(n[0] >= "wide_header".chars().count());
    assert!(n[1] >= "loooooong".chars().count());
}

#[test]
fn contents_natural_widths_parallel_path_deterministic() {
    let row = json!({"c": "cell"});
    let entries: Vec<Value> = (0..PARALLEL.contents_natural_widths + 50)
        .map(|_| row.clone())
        .collect();
    let section = ublx::render::kv_tables::ContentsSection {
        title: "Big".to_string(),
        columns: vec!["C".to_string()],
        column_keys: vec!["c".to_string()],
        entries,
        sub_title: false,
    };
    let start = 0;
    let end = PARALLEL.contents_natural_widths + 20;
    let a = contents_natural_widths(&section, start, end, DEFAULT_MAX_ARRAY_INLINE);
    let b = contents_natural_widths(&section, start, end, DEFAULT_MAX_ARRAY_INLINE);
    assert_eq!(a, b);
    assert_eq!(a.len(), 1);
}

#[test]
fn column_stats_abbrev_caps_long_tables() {
    use ublx::config::ColumnStatsDisplay;
    use ublx::render::kv_tables::{Section, parse_json_sections_with};
    let mut columns = String::from("[");
    for i in 0..20 {
        if i > 0 {
            columns.push(',');
        }
        columns.push_str(&format!(
            r#"{{"i": {i}, "name": "col_{i}", "t": "string", "null_pct": 0.0}}"#
        ));
    }
    columns.push(']');
    let json = format!(r#"{{"row_count": 100, "columns": {columns}}}"#);
    let abbrev = parse_json_sections_with(&json, 80, ColumnStatsDisplay::Abbrev);
    let full = parse_json_sections_with(&json, 80, ColumnStatsDisplay::Full);
    let abbrev_table = abbrev
        .iter()
        .find_map(|s| match s {
            Section::Contents(c) => Some(c),
            _ => None,
        })
        .expect("string columns table");
    let full_table = full
        .iter()
        .find_map(|s| match s {
            Section::Contents(c) => Some(c),
            _ => None,
        })
        .expect("string columns table");
    assert_eq!(abbrev_table.entries.len(), 10);
    assert!(abbrev_table.title.contains("(10/20)"));
    assert_eq!(full_table.entries.len(), 20);
}

#[test]
fn column_stats_abbrev_keeps_all_stat_columns_on_short_table() {
    use ublx::config::ColumnStatsDisplay;
    use ublx::render::kv_tables::{Section, parse_json_sections_with};
    let json = r#"{
        "row_count": 100,
        "columns": [
            {"i": 0, "name": "x", "t": "number", "null_pct": 0.0,
             "num": {"min": 1.0, "max": 9.0, "mean": 5.0, "median": 5.0, "stdev": 2.0}}
        ]
    }"#;
    let sections = parse_json_sections_with(json, 80, ColumnStatsDisplay::Abbrev);
    let contents = sections
        .iter()
        .find_map(|s| match s {
            Section::Contents(c) => Some(c),
            _ => None,
        })
        .expect("number columns table");
    assert_eq!(contents.entries.len(), 1);
    assert!(contents.column_keys.iter().any(|k| k == "median"));
    assert!(contents.column_keys.iter().any(|k| k == "stdev"));
}

#[test]
fn column_stats_none_skips_tables() {
    use ublx::config::ColumnStatsDisplay;
    use ublx::render::kv_tables::{Section, parse_json_sections_with};
    let json = r#"{
        "row_count": 100,
        "columns": [
            {"i": 0, "name": "x", "t": "number", "num": {"min": 1.0, "max": 9.0, "mean": 5.0}}
        ]
    }"#;
    let sections = parse_json_sections_with(json, 80, ColumnStatsDisplay::None);
    assert!(sections.iter().all(|s| !matches!(s, Section::Contents(_))));
}
