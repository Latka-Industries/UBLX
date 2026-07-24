//! Code-based syntax highlighting for the Viewer tab.
//!
//! **Whether** to highlight comes only from the snapshot [`UblxDbCategory`] (same strings as the DB
//! `category` column). **Which** syntect grammar to use follows that type; for [`ZahirFT::Code`] the
//! path/filename selects the language (Rust vs TypeScript, etc.), since the DB only stores “Code”.
//!
//! Extra grammars (e.g. TOML, TypeScript) come from the `sublime_syntaxes` crate (bat-sourced
//! `.sublime-syntax` blobs), consulted after [`SyntaxSet::load_defaults_newlines`].

use std::path::Path;
use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style as RatStyle};
use ratatui::text::{Line, Span, Text};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style as SynStyle};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::engine::db_ops::UblxDbCategory;
use crate::integrations::ZahirFT;
use crate::render::viewers::html_escape::html_escape_minimal;
use crate::themes::{self, Palette};

static DEFAULT_SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static EXTRA_SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(sublime_syntaxes::extra_syntax_set);

/// Plain text and fallback always use the default pack (both sets carry a “Plain Text” entry;
/// defaults stay canonical).
fn plain(default: &SyntaxSet) -> (&SyntaxSet, &SyntaxReference) {
    (default, default.find_syntax_plain_text())
}

fn find_by_extension<'a>(
    default: &'a SyntaxSet,
    extra: &'a SyntaxSet,
    ext: &str,
) -> Option<(&'a SyntaxSet, &'a SyntaxReference)> {
    default
        .find_syntax_by_extension(ext)
        .map(|s| (default, s))
        .or_else(|| extra.find_syntax_by_extension(ext).map(|s| (extra, s)))
}

fn find_by_first_line<'a>(
    default: &'a SyntaxSet,
    extra: &'a SyntaxSet,
    line: &str,
) -> Option<(&'a SyntaxSet, &'a SyntaxReference)> {
    default
        .find_syntax_by_first_line(line)
        .map(|s| (default, s))
        .or_else(|| extra.find_syntax_by_first_line(line).map(|s| (extra, s)))
}

fn pick_syntax_by_path<'a>(
    default: &'a SyntaxSet,
    extra: &'a SyntaxSet,
    path: &str,
    raw: &str,
) -> (&'a SyntaxSet, &'a SyntaxReference) {
    let first_line = raw.lines().next().unwrap_or("");
    let p = Path::new(path);
    let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    find_by_extension(default, extra, fname)
        .or_else(|| find_by_extension(default, extra, ext))
        .or_else(|| find_by_first_line(default, extra, first_line))
        .unwrap_or_else(|| plain(default))
}

fn syn_style_to_ratatui(s: &SynStyle) -> RatStyle {
    let fg = s.foreground;
    let mut st = RatStyle::default().fg(Color::Rgb(fg.r, fg.g, fg.b));
    let fs = s.font_style;
    if fs.contains(FontStyle::BOLD) {
        st = st.add_modifier(Modifier::BOLD);
    }
    if fs.contains(FontStyle::ITALIC) {
        st = st.add_modifier(Modifier::ITALIC);
    }
    if fs.contains(FontStyle::UNDERLINE) {
        st = st.add_modifier(Modifier::UNDERLINED);
    }
    st
}

fn pick_syntax<'a>(
    default: &'a SyntaxSet,
    extra: &'a SyntaxSet,
    ft: ZahirFT,
    path: &str,
    raw: &str,
) -> (&'a SyntaxSet, &'a SyntaxReference) {
    let first_line = raw.lines().next().unwrap_or("");
    match ft {
        ZahirFT::Json => {
            find_by_extension(default, extra, "json").unwrap_or_else(|| plain(default))
        }
        ZahirFT::Toml => {
            find_by_extension(default, extra, "toml").unwrap_or_else(|| plain(default))
        }
        ZahirFT::Yaml => {
            find_by_extension(default, extra, "yaml").unwrap_or_else(|| plain(default))
        }
        ZahirFT::Xml => find_by_extension(default, extra, "xml").unwrap_or_else(|| plain(default)),
        ZahirFT::Html => {
            find_by_extension(default, extra, "html").unwrap_or_else(|| plain(default))
        }
        ZahirFT::Ini => find_by_extension(default, extra, "ini").unwrap_or_else(|| plain(default)),
        ZahirFT::Log => find_by_extension(default, extra, "log")
            .or_else(|| find_by_first_line(default, extra, first_line))
            .unwrap_or_else(|| plain(default)),
        ZahirFT::Code => pick_syntax_by_path(default, extra, path, raw),
        _ => plain(default),
    }
}

/// Snapshot categories that use syntect in the Viewer (same set as TUI `viewer_uses_syntect_highlight`).
#[must_use]
pub fn uses_syntect_ft(ft: ZahirFT) -> bool {
    matches!(
        ft,
        ZahirFT::Json
            | ZahirFT::Toml
            | ZahirFT::Yaml
            | ZahirFT::Xml
            | ZahirFT::Html
            | ZahirFT::Ini
            | ZahirFT::Log
            | ZahirFT::Code
    )
}

/// Host HTML for web Viewer — same grammar/theme pick as [`highlight_viewer_for_palette`].
#[must_use]
pub fn highlight_viewer_html(raw: &str, path: &str, ft: ZahirFT, palette: &Palette) -> String {
    let default = &*DEFAULT_SYNTAX_SET;
    let extra = &*EXTRA_SYNTAX_SET;
    let (ss, syntax) = pick_syntax(default, extra, ft, path, raw);
    let theme = themes::theme_for_palette(palette);
    match syntect::html::highlighted_html_for_string(raw, ss, syntax, theme) {
        Ok(html) => html,
        Err(_) => format!("<pre>{}</pre>", html_escape_minimal(raw)),
    }
}

/// Syntax-highlight using DB [`UblxDbCategory`]; caller should only invoke for zahir types that use syntect.
#[must_use]
pub fn highlight_viewer(raw: &str, path: &str, cat: UblxDbCategory) -> Text<'static> {
    highlight_viewer_for_palette(raw, path, cat, themes::current())
}

/// Same as [`highlight_viewer`], but with an explicit palette (e.g. background worker without `themes::current()`).
#[must_use]
pub fn highlight_viewer_for_palette(
    raw: &str,
    path: &str,
    cat: UblxDbCategory,
    palette: &Palette,
) -> Text<'static> {
    let UblxDbCategory::Zahir(ft) = cat else {
        return Text::from(raw.to_string());
    };
    let default = &*DEFAULT_SYNTAX_SET;
    let extra = &*EXTRA_SYNTAX_SET;
    let (ss, syntax) = pick_syntax(default, extra, ft, path, raw);
    let theme = themes::theme_for_palette(palette);
    let mut h = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();
    for line in LinesWithEndings::from(raw) {
        match h.highlight_line(line, ss) {
            Ok(regions) => {
                let mut spans = Vec::new();
                for (style, text) in regions {
                    if text.is_empty() {
                        continue;
                    }
                    spans.push(Span::styled(text.to_string(), syn_style_to_ratatui(&style)));
                }
                lines.push(if spans.is_empty() {
                    Line::default()
                } else {
                    Line::from(spans)
                });
            }
            Err(_) => {
                return Text::from(raw.to_string());
            }
        }
    }
    Text::from(lines)
}
