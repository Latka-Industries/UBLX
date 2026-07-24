//! Tiny async helpers for the chord timer / toasts.

use crate::api::{SettingsView, ThemeCssBody, ThemePickerRow};

use super::ctx::CommandModeCtx;

pub(super) use crate::util::sleep_ms;

/// Flash API errors, or run `on_ok` and flash its message on success.
pub(super) async fn flash_api<T>(
    ctx: CommandModeCtx,
    result: Result<T, String>,
    on_ok: impl FnOnce(T) -> String,
) {
    match result {
        Ok(v) => ctx.flash(on_ok(v)),
        Err(e) => ctx.flash_err(e),
    }
}

/// Flash API errors, or run side effects and flash a fixed message on success.
pub(super) async fn flash_api_side_effect<T>(
    ctx: CommandModeCtx,
    result: Result<T, String>,
    on_ok: impl FnOnce(T),
    msg: &'static str,
) {
    match result {
        Ok(v) => {
            on_ok(v);
            ctx.flash(msg);
        }
        Err(e) => ctx.flash_err(e),
    }
}

/// Theme picker rows from settings, with legacy `themes`-only fallback.
pub(super) fn theme_picker_rows(v: &SettingsView) -> Vec<ThemePickerRow> {
    if v.theme_picker.is_empty() {
        // Fallback if an older serve omits theme_picker.
        v.themes
            .iter()
            .map(|name| ThemePickerRow::Theme {
                name: name.clone(),
                appearance: String::new(),
                swatch: String::new(),
                css: ThemeCssBody::default(),
            })
            .collect()
    } else {
        v.theme_picker.clone()
    }
}
