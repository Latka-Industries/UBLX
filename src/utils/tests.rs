//! Shared helpers: formatting, clipboard detect, theme lightening.
//! Path helpers live in [`crate::utils::path`] unit tests.

use crate::render::viewers::markdown::is_markdown_path;
use crate::themes::{Appearance, adjust_surface_rgb};
use crate::utils::ClipboardCopyCommand;
use crate::utils::{
    clamp_selection, clamp_selection_opt, format_timestamp_ns, frame_string_with_spaces,
    truncate_middle,
};
use ratatui::style::Color;

#[test]
fn clamp_selection_in_range() {
    assert_eq!(clamp_selection(0, 5), 0);
    assert_eq!(clamp_selection(2, 5), 2);
    assert_eq!(clamp_selection(4, 5), 4);
}

#[test]
fn clamp_selection_over_max() {
    assert_eq!(clamp_selection(5, 5), 4);
    assert_eq!(clamp_selection(10, 3), 2);
}

#[test]
fn clamp_selection_empty_list() {
    assert_eq!(clamp_selection(0, 0), 0);
    assert_eq!(clamp_selection(3, 0), 0);
}

#[test]
fn clamp_selection_opt_some() {
    assert_eq!(clamp_selection_opt(1, 5), Some(1));
    assert_eq!(clamp_selection_opt(4, 5), Some(4));
    assert_eq!(clamp_selection_opt(10, 5), Some(4));
}

#[test]
fn clamp_selection_opt_none() {
    assert_eq!(clamp_selection_opt(0, 0), None);
    assert_eq!(clamp_selection_opt(3, 0), None);
}

#[test]
fn test_frame_string_with_spaces() {
    assert_eq!(frame_string_with_spaces("Delta"), " Delta ");
    assert_eq!(frame_string_with_spaces(""), "  ");
}

#[test]
fn truncate_middle_short() {
    assert_eq!(truncate_middle("short", 10), "short");
    assert_eq!(truncate_middle("ab", 3), "ab");
}

#[test]
fn truncate_middle_long() {
    let s = truncate_middle("hello world", 8);
    assert_eq!(s.len(), 8);
    assert!(s.contains("..."));
}

#[test]
fn format_timestamp_ns_valid() {
    let s = format_timestamp_ns(1_000_000_000);
    assert!(
        !s.contains("invalid"),
        "expected valid timestamp string, got {s:?}"
    );
    assert!(
        s.chars().filter(|c| c.is_ascii_digit()).count() >= 8,
        "expected digits in output: {s:?}"
    );
}

#[test]
fn format_timestamp_ns_negative_no_panic() {
    let s = format_timestamp_ns(-1);
    assert!(!s.is_empty());
}

#[test]
fn is_markdown_path_matches_extensions() {
    assert!(is_markdown_path("README.md"));
    assert!(is_markdown_path("notes.MARKDOWN"));
    assert!(is_markdown_path("deep/path/file.md"));
}

#[test]
fn is_markdown_path_rejects_others() {
    assert!(!is_markdown_path("file.txt"));
    assert!(!is_markdown_path("noext"));
    assert!(!is_markdown_path("foo.md.backup"));
}

#[test]
fn detect_returns_some_on_macos_or_when_tools_exist() {
    let c = ClipboardCopyCommand::detect();
    if cfg!(target_os = "macos") {
        assert!(c.is_some(), "pbcopy should be present on macOS");
    }
    if let Some(cmd) = c {
        assert!(!cmd.argv.is_empty());
    }
}

/// Run with: `cargo test print_lighten_values -- --nocapture` to dump RGB values for palette work.
#[test]
fn print_lighten_values() {
    // Same dark-then-light / alphabetical order as `theme_ordered_list` in palettes.rs
    let backgrounds: &[(&str, u8, u8, u8)] = &[
        ("Archival Simulacra", 0, 0, 0),
        ("Babel Blend", 12, 22, 46),
        ("Burning Glyph", 42, 0, 0),
        ("Frozen Phrase", 46, 52, 64),
        ("Garden Unseen", 0, 42, 21),
        ("Golden Delirium", 42, 42, 0),
        ("Oblivion Ink", 10, 25, 47),
        ("Purple Haze", 13, 0, 26),
        ("Resin Record", 42, 18, 0),
        ("Shadow Index", 0, 0, 0),
        ("Tangerine Memory", 42, 26, 0),
        ("Asterion Code", 232, 240, 242),
        ("Barley Bound", 251, 241, 199),
        ("Verglas Trace", 236, 239, 244),
        ("Cryptic Chai", 247, 238, 222),
        ("Faded Echo", 246, 237, 225),
        ("Infinite Rose", 232, 235, 240),
        ("Obdurate Noon", 253, 246, 227),
        ("Ochre Thread", 250, 242, 228),
        ("Pale Mirror", 242, 245, 253),
        ("Parched Page", 253, 248, 240),
        ("Silent Sheet", 255, 255, 255),
    ];
    for (name, r, g, b) in backgrounds {
        let bg = Color::Rgb(*r, *g, *b);
        let appearance = if matches!(
            *name,
            "Silent Sheet"
                | "Parched Page"
                | "Pale Mirror"
                | "Ochre Thread"
                | "Obdurate Noon"
                | "Infinite Rose"
                | "Faded Echo"
                | "Cryptic Chai"
                | "Asterion Code"
                | "Barley Bound"
                | "Verglas Trace"
        ) {
            Appearance::Light
        } else {
            Appearance::Dark
        };
        let popup = adjust_surface_rgb(bg, 0.05, appearance);
        let node = adjust_surface_rgb(bg, 0.10, appearance);
        let Color::Rgb(pr, pg, pb) = popup else {
            unreachable!("adjust_surface_rgb preserves Rgb for Rgb input")
        };
        let Color::Rgb(nr, ng, nb) = node else {
            unreachable!("adjust_surface_rgb preserves Rgb for Rgb input")
        };
        println!(
            "{}: popup/notif Color::Rgb({}, {}, {}), node_bg Color::Rgb({}, {}, {})",
            name, pr, pg, pb, nr, ng, nb
        );
    }
}
