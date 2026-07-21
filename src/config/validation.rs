//! Validation for hot-reloadable config. Variable-specific errors so the user sees which field failed and why.

use super::profile::UblxOverlay;

/// Maximum value for layout pane percentages (`left_pct`, `middle_pct`, `right_pct`). Each must be 0–100 and they must sum to this.
pub const LAYOUT_PCT_MAX: u16 = 100;

/// One validation failure: which field and a short message.
#[derive(Clone, Debug)]
pub struct HotReloadError {
    pub field: &'static str,
    pub message: String,
}

impl std::fmt::Display for HotReloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Format all validation errors for display (numbered, newline-separated). Used for startup and hot-reload toasts.
#[must_use]
pub fn first_validation_error_message(errors: &[HotReloadError]) -> String {
    if errors.is_empty() {
        "invalid config".to_string()
    } else {
        let n = errors.len();
        let numbered = errors
            .iter()
            .enumerate()
            .map(|(i, e)| format!("{}) {}", i + 1, e))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Found {n} error(s):\n{numbered}")
    }
}

/// Result of a hot-reload attempt: whether an overlay was applied and any validation errors.
#[derive(Clone, Debug, Default)]
pub struct ReloadResult {
    /// True if the overlay was valid and applied.
    pub applied: bool,
    /// Non-empty when validation failed (overlay was not applied).
    pub validation_errors: Vec<HotReloadError>,
}

/// Validates only the hot-reloadable parts of the overlay. Call before applying.
/// `valid_theme_names`: allowed values for `theme` (e.g. from [`crate::themes::theme_ordered_list`] names).
///
/// # Errors
///
/// Returns `Err(validation_errors)` when the overlay fails validation (theme, layout percentages, etc.).
pub fn validate_hot_reload_overlay(
    overlay: &UblxOverlay,
    valid_theme_names: &[&str],
) -> Result<(), Vec<HotReloadError>> {
    let mut errors = Vec::new();

    if let Some(ref name) = overlay.theme {
        let name = name.trim();
        if !name.is_empty() && !valid_theme_names.contains(&name) {
            errors.push(HotReloadError {
                field: "theme",
                message: format!("invalid: \"{name}\"; run `ublx --themes` to list valid options"),
            });
        }
    }

    if let Some(ref layout) = overlay.layout {
        for (field, pct) in [
            ("layout.left_pct", layout.left_pct),
            ("layout.middle_pct", layout.middle_pct),
            ("layout.right_pct", layout.right_pct),
        ] {
            if pct > LAYOUT_PCT_MAX {
                errors.push(HotReloadError {
                    field,
                    message: format!("must be 0–{LAYOUT_PCT_MAX} (got {pct})"),
                });
            }
        }
        let sum =
            u32::from(layout.left_pct) + u32::from(layout.middle_pct) + u32::from(layout.right_pct);
        if errors.iter().all(|e| !e.field.starts_with("layout")) && sum != u32::from(LAYOUT_PCT_MAX)
        {
            errors.push(HotReloadError {
                field: "layout",
                message: format!(
                    "left_pct + middle_pct + right_pct must = {LAYOUT_PCT_MAX} (got {sum})"
                ),
            });
        }
    }

    if let Some(a) = overlay.bg_opacity
        && (!a.is_finite() || !(0.0..=1.0).contains(&a))
    {
        errors.push(HotReloadError {
            field: "bg_opacity",
            message: format!("must be between 0.0 and 1.0 (got {a})"),
        });
    }

    if let Some(ref cm) = overlay.command_mode
        && let Some(ref leader) = cm.leader
        && let Err(message) = super::parse_command_mode_leader(leader)
    {
        errors.push(HotReloadError {
            field: "command_mode.leader",
            message,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
