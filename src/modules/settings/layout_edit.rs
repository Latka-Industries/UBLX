//! Layout triplet (left/middle/right %) and background opacity: cursor bounds, digit buffers, parse.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::layout::setup::{SettingsConfigScope, SettingsPaneState, UblxState};
use crate::utils::clamp_selection;

use super::typed_column_tables_row::typed_column_tables_row_index;

/// Row index for OSC 11 opacity payload format (`rgba` | `hex8`), after bool rows + `typed_column_tables`
/// (+ Command Mode leader on Global).
/// [`None`] on Local — that row exists only on the Global settings tab.
#[must_use]
pub fn opacity_format_row_index(scope: SettingsConfigScope) -> Option<usize> {
    match scope {
        SettingsConfigScope::Global => Some(typed_column_tables_row_index(scope) + 2),
        SettingsConfigScope::Local => None,
    }
}

/// "Edit layout" button row (after bool rows + `typed_column_tables` on Local; + command mode + opacity format on Global).
#[must_use]
pub fn layout_button_index(scope: SettingsConfigScope) -> usize {
    match scope {
        SettingsConfigScope::Global => typed_column_tables_row_index(scope) + 3,
        SettingsConfigScope::Local => typed_column_tables_row_index(scope) + 1,
    }
}

/// First row after the layout block (opacity button).
#[must_use]
pub fn opacity_button_index(settings_ref: &SettingsPaneState, scope: SettingsConfigScope) -> usize {
    let b = layout_button_index(scope);
    b + 1 + if settings_ref.layout_unlocked { 3 } else { 0 }
}

#[must_use]
pub fn max_left_cursor(settings_ref: &SettingsPaneState, scope: SettingsConfigScope) -> usize {
    let ob = opacity_button_index(settings_ref, scope);
    if settings_ref.opacity_unlocked {
        ob + 1
    } else {
        ob
    }
}

#[must_use]
pub fn left_cursor_len(settings_ref: &SettingsPaneState, scope: SettingsConfigScope) -> usize {
    max_left_cursor(settings_ref, scope) + 1
}

pub fn bump_settings_cursor(state_mut: &mut UblxState, scope: SettingsConfigScope, down: bool) {
    let n = left_cursor_len(&state_mut.settings, scope);
    if down {
        state_mut.settings.left_cursor = clamp_selection(state_mut.settings.left_cursor + 1, n);
    } else {
        state_mut.settings.left_cursor =
            clamp_selection(state_mut.settings.left_cursor.saturating_sub(1), n);
    }
}

#[must_use]
pub fn parse_layout_triplet(
    left_ref: &str,
    mid_ref: &str,
    right_ref: &str,
) -> Option<(u16, u16, u16)> {
    let l: u16 = left_ref.trim().parse().ok()?;
    let m: u16 = mid_ref.trim().parse().ok()?;
    let r: u16 = right_ref.trim().parse().ok()?;
    if l + m + r != 100 {
        return None;
    }
    Some((l, m, r))
}

/// Parse `bg_opacity` buffer: `0.0`–`1.0`. Empty is invalid at save time.
#[must_use]
pub fn parse_bg_opacity(buf: &str) -> Option<f32> {
    let s = buf.trim();
    if s.is_empty() {
        return None;
    }
    let v: f32 = s.parse().ok()?;
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        return None;
    }
    Some(v)
}

/// Raw key for layout digit editing when unlocked and a layout field is focused.
pub fn handle_layout_text_key(state_mut: &mut UblxState, key: crossterm::event::KeyEvent) -> bool {
    if !state_mut.settings.layout_unlocked {
        return false;
    }
    let scope = state_mut.settings.scope;
    let btn = layout_button_index(scope);
    let cur = state_mut.settings.left_cursor;
    if cur <= btn || cur > btn + 3 {
        return false;
    }
    let field = cur - btn - 1;
    let buf = match field {
        0 => &mut state_mut.settings.layout_left_buf,
        1 => &mut state_mut.settings.layout_mid_buf,
        2 => &mut state_mut.settings.layout_right_buf,
        _ => return false,
    };
    match key.code {
        KeyCode::Char(c) if key.modifiers.intersection(KeyModifiers::CONTROL).is_empty() => {
            if c.is_ascii_digit() && buf.len() < 3 {
                buf.push(c);
            }
            true
        }
        KeyCode::Backspace => {
            buf.pop();
            true
        }
        _ => false,
    }
}

/// Raw key for opacity field (`0.0`–`1.0`, optional `.`).
pub fn handle_opacity_text_key(state_mut: &mut UblxState, key: crossterm::event::KeyEvent) -> bool {
    if !state_mut.settings.opacity_unlocked {
        return false;
    }
    let scope = state_mut.settings.scope;
    let ob = opacity_button_index(&state_mut.settings, scope);
    let cur = state_mut.settings.left_cursor;
    if cur != ob + 1 {
        return false;
    }
    let buf = &mut state_mut.settings.opacity_buf;
    match key.code {
        KeyCode::Char(c) if key.modifiers.intersection(KeyModifiers::CONTROL).is_empty() => {
            if buf.len() >= 8 {
                return true;
            }
            match c {
                '0'..='9' => buf.push(c),
                '.' if !buf.contains('.') => buf.push(c),
                _ => {}
            }
            true
        }
        KeyCode::Backspace => {
            buf.pop();
            true
        }
        _ => false,
    }
}
