//! Row labels — mirror TUI `COMMAND_MODE_DESCRIPTIONS` / `CTRL_MENU_ROWS`.

pub(crate) const DEFAULT_LEADER: char = 'a';

/// Menu delay before showing the centered table (TUI `CHORD_MENU_DELAY`).
pub(crate) const CHORD_MENU_DELAY_MS: i32 = 480;

pub(crate) const COMMAND_MODE_ROWS: &[(&str, &str)] = &[
    ("d", "Run duplicate detection"),
    ("t", "Theme selector"),
    ("s", "Take snapshot"),
    ("r", "Reload config from disk"),
    ("x", "Export Zahir JSON (ublx-export/)"),
    ("l", "Export lenses as Markdown (ublx-lenses/)"),
    ("p", "Switch UBLX project"),
];

pub(crate) fn popup_title(leader: char) -> String {
    format!("Command Mode (Ctrl+{leader})")
}

pub(crate) fn description_for(key: char) -> Option<&'static str> {
    COMMAND_MODE_ROWS
        .iter()
        .find(|(k, _)| k.starts_with(key))
        .map(|(_, d)| *d)
}
