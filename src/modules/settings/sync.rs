//! ublx-settings applet: first-tick toast and config file watcher reload.

use std::time::Duration;

use crate::app::RunUblxParams;
use crate::config::{OPERATION_NAME, UblxOpts, UblxPaths, first_validation_error_message};
use crate::layout::setup::UblxState;
use crate::themes;
use crate::ui::{UI_STRINGS, show_operation_toast};
use crate::utils::{self, sync_osc11_page_background};

/// Window (ms) after we write config ourselves (e.g. theme selector) during which a file-watcher reload is treated as self-caused.
const CONFIG_SELF_WRITE_WINDOW_MS: u64 = 800;

/// When a hot reload turns a snapshot-affecting bool from off→on, stash the prior cached value so the next snapshot still sees a false→true flip (see [`crate::config::UblxOpts`] `*_cache_before_apply` fields).
fn set_snapshot_cache_before_apply_on_flip_to_true(flip_on: bool, cache: &mut Option<bool>) {
    if flip_on {
        *cache = Some(false);
    }
}

/// Show ublx-settings toast on first tick (e.g. config loaded / validation message from startup).
pub fn on_first_tick(state_mut: &mut UblxState, params_ref: &RunUblxParams<'_>) {
    if !state_mut.session.tick.first_tick {
        return;
    }
    state_mut.session.tick.first_tick = false;
    if let Some(b) = params_ref.bumper {
        let op = OPERATION_NAME.op("settings");
        utils::show_toast_slot(
            &mut state_mut.toasts.slots,
            b,
            Some(op.as_str()),
            &mut state_mut.toasts.consumed_per_operation,
        );
    }
}

/// Copy theme / layout / background opacity from [`UblxOpts`] into [`RunUblxParams`] after reload, and refresh OSC 11.
pub fn sync_run_params_from_opts(
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_ref: &UblxOpts,
    state_mut: &mut UblxState,
) {
    params_mut.theme.clone_from(&ublx_opts_ref.theme);
    params_mut.layout.clone_from(&ublx_opts_ref.layout);
    params_mut.bg_opacity = ublx_opts_ref.bg_opacity.unwrap_or(1.0);
    params_mut.opacity_format = ublx_opts_ref.opacity_format;
    ublx_opts_ref.sync_panels_display(&mut state_mut.panels);
    let _ = sync_osc11_page_background(
        params_mut.theme.as_deref(),
        params_mut.bg_opacity,
        params_mut.opacity_format,
    );
}

/// If config watcher fired: optionally clear theme override (if external save), then apply reload and optional toast.
///
/// Self-caused writes (Settings / theme selector) suppress the "Config reloaded" toast via
/// [`UblxState::config_written_by_us_at`]; only external disk changes toast.
pub fn on_config_reload(
    state_mut: &mut UblxState,
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
) {
    let from_external_save = state_mut
        .config_written_by_us_at
        .as_ref()
        .is_none_or(|t| t.elapsed() >= Duration::from_millis(CONFIG_SELF_WRITE_WINDOW_MS));
    if from_external_save {
        state_mut.theme.override_name = None;
    }
    let toast = from_external_save.then_some(UI_STRINGS.toasts.config_reloaded);
    apply_config_reload(params_mut, ublx_opts_mut, state_mut, toast);
}

/// Reloads hot-reloadable config from paths and syncs theme/layout into params. Validates before applying; on validation failure shows a toast with variable-specific errors. If applied and `message` is `Some`, shows success toast (use `None` when the change was caused by us, e.g. theme selector write).
///
/// Flipping `enable_enhance_all` or `hash` to `true` only updates cache-before-apply flags so the **next** snapshot run picks up full Zahir / hashing (same idea as `show_hidden_files`); it does not queue a background snapshot.
pub fn apply_config_reload(
    params_mut: &mut RunUblxParams<'_>,
    ublx_opts_mut: &mut UblxOpts,
    state_mut: &mut UblxState,
    message: Option<impl AsRef<str>>,
) {
    let paths = UblxPaths::new(&params_mut.dir_to_ublx);
    let valid_themes: Vec<&str> = themes::theme_ordered_list()
        .iter()
        .map(|t| t.name)
        .collect();
    let old_enable_enhance_all = ublx_opts_mut.enable_enhance_all;
    let old_with_hash = ublx_opts_mut.nefax_opts.with_hash;
    let result = ublx_opts_mut.reload_hot_config(&paths, &valid_themes);

    if result.applied {
        sync_run_params_from_opts(params_mut, ublx_opts_mut, state_mut);
        set_snapshot_cache_before_apply_on_flip_to_true(
            !old_enable_enhance_all && ublx_opts_mut.enable_enhance_all,
            &mut ublx_opts_mut.enable_enhance_all_cache_before_apply,
        );
        set_snapshot_cache_before_apply_on_flip_to_true(
            !old_with_hash && ublx_opts_mut.nefax_opts.with_hash,
            &mut ublx_opts_mut.with_hash_cache_before_apply,
        );
        if let Some(msg) = message {
            show_operation_toast(state_mut, params_mut, msg, "settings", log::Level::Info);
        }
    } else if !result.validation_errors.is_empty() {
        sync_run_params_from_opts(params_mut, ublx_opts_mut, state_mut);
        let msg = first_validation_error_message(&result.validation_errors);
        let warn_msg = format!("Config validation: {msg}");
        show_operation_toast(
            state_mut,
            params_mut,
            warn_msg,
            "settings",
            log::Level::Warn,
        );
    }
}
