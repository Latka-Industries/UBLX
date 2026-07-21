//! Command Mode leader letter (`Ctrl+{letter}`). Global-only hot-reloadable config.

use super::profile::{CommandModeOverlay, UblxOverlay};

/// Default leader when config omits `[command_mode].leader`.
pub const DEFAULT_COMMAND_MODE_LEADER: char = 'a';

const LEADER_RULE: &str = "a–z (not j/k)";

/// Letters reserved for other Ctrl chords (`Ctrl+j` / `Ctrl+k` jump-by-10).
#[must_use]
pub fn is_reserved_command_mode_leader(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), 'j' | 'k')
}

/// True for a single ASCII letter that can be the Command Mode leader.
#[must_use]
pub fn is_valid_command_mode_leader(c: char) -> bool {
    c.is_ascii_alphabetic() && !is_reserved_command_mode_leader(c)
}

/// Lowercase valid letter, or [`DEFAULT_COMMAND_MODE_LEADER`] when invalid.
#[must_use]
pub fn normalize_command_mode_leader(c: char) -> char {
    if is_valid_command_mode_leader(c) {
        c.to_ascii_lowercase()
    } else {
        DEFAULT_COMMAND_MODE_LEADER
    }
}

/// Human-readable reason when `c` cannot be a leader (`None` if valid).
#[must_use]
pub fn command_mode_leader_reject_reason(c: char) -> Option<&'static str> {
    let c = c.to_ascii_lowercase();
    if is_valid_command_mode_leader(c) {
        None
    } else if is_reserved_command_mode_leader(c) {
        Some("j and k are reserved for Ctrl+j / Ctrl+k jump")
    } else {
        Some("must be a letter a–z (not j/k)")
    }
}

/// Parse TOML `leader = "…"` into a lowercase ASCII letter.
///
/// # Errors
///
/// Returns a short message when the value is not a single allowed letter.
pub fn parse_command_mode_leader(s: &str) -> Result<char, String> {
    let t = s.trim();
    let mut chars = t.chars();
    let Some(c) = chars.next() else {
        return Err(format!("must be a single letter {LEADER_RULE}"));
    };
    if chars.next().is_some() {
        return Err(format!(
            "must be a single letter {LEADER_RULE}; got \"{t}\""
        ));
    }
    if let Some(reason) = command_mode_leader_reject_reason(c) {
        return Err(if reason.starts_with("must be") {
            format!("{reason}; got \"{t}\"")
        } else {
            reason.to_string()
        });
    }
    Ok(c.to_ascii_lowercase())
}

/// Effective leader from an overlay (`a` when unset / invalid at apply time after validation).
#[must_use]
pub fn overlay_command_mode_leader(overlay: &UblxOverlay) -> char {
    overlay
        .command_mode
        .as_ref()
        .and_then(|cm| cm.leader.as_deref())
        .and_then(|s| parse_command_mode_leader(s).ok())
        .unwrap_or(DEFAULT_COMMAND_MODE_LEADER)
}

/// Next letter after `current` in a–z, skipping reserved.
#[must_use]
pub fn cycle_command_mode_leader(current: char) -> char {
    let mut c = normalize_command_mode_leader(current);
    for _ in 0..26 {
        c = if c == 'z' {
            'a'
        } else {
            char::from((c as u8) + 1)
        };
        if is_valid_command_mode_leader(c) {
            return c;
        }
    }
    DEFAULT_COMMAND_MODE_LEADER
}

/// Popup / help title: `Command Mode (Ctrl+a)`.
#[must_use]
pub fn command_mode_popup_title(leader: char) -> String {
    format!(
        "Command Mode (Ctrl+{})",
        normalize_command_mode_leader(leader)
    )
}

/// Write leader into overlay `[command_mode]` section.
pub fn write_command_mode_leader(overlay: &mut UblxOverlay, leader: char) {
    let c = normalize_command_mode_leader(leader);
    let cm = overlay
        .command_mode
        .get_or_insert_with(CommandModeOverlay::default);
    cm.leader = Some(c.to_string());
}
