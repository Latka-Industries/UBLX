//! Run Command Mode letter actions against serve APIs.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{
    SettingsPatch, SettingsScope, fetch_duplicates, fetch_roots, fetch_settings,
    get_snapshot_status, patch_settings, post_export_lenses, post_export_zahir, post_snapshot,
    switch_root,
};
use crate::nav::MainMode;
use crate::theme::apply_theme_css_body;

use super::ctx::{CommandModeCtx, Picker};
use super::helpers::{flash_api, flash_api_side_effect, sleep_ms, theme_picker_rows};

pub(super) fn run_letter(ctx: CommandModeCtx, c: char) {
    let c = c.to_ascii_lowercase();
    match c {
        'd' => spawn_local(run_duplicates(ctx)),
        't' => open_theme_selector(ctx, SettingsScope::Local),
        's' => spawn_local(run_snapshot(ctx)),
        'r' => spawn_local(run_reload(ctx)),
        'x' => spawn_local(run_export_zahir(ctx)),
        'l' => spawn_local(run_export_lenses(ctx)),
        'p' => spawn_local(open_root_picker(ctx)),
        _ => {}
    }
}

pub(super) fn open_theme_selector(ctx: CommandModeCtx, scope: SettingsScope) {
    ctx.theme_scope.set(scope);
    spawn_local(open_theme_picker(ctx));
}

async fn run_duplicates(ctx: CommandModeCtx) {
    ctx.flash("Scanning for duplicates…");
    let resp = fetch_duplicates().await;
    ctx.refresh.bump();
    if resp.groups.is_empty() {
        ctx.flash("No duplicates found");
    } else {
        ctx.set_mode.set(MainMode::Duplicates);
        ctx.flash(format!("Duplicates: {} group(s)", resp.groups.len()));
    }
}

async fn run_snapshot(ctx: CommandModeCtx) {
    match post_snapshot(false).await {
        Ok(_) => {
            ctx.flash("Snapshot started…");
            poll_snapshot(ctx).await;
        }
        Err(e) => ctx.flash(e),
    }
}

async fn poll_snapshot(ctx: CommandModeCtx) {
    for _ in 0..600 {
        sleep_ms(500).await;
        match get_snapshot_status().await {
            Ok(st) if st.state.eq_ignore_ascii_case("running") => continue,
            Ok(st) if st.state.eq_ignore_ascii_case("done") => {
                ctx.refresh.bump();
                let msg = st
                    .last
                    .map(|l| format!("Snapshot done +{} ~{} -{}", l.added, l.modified, l.removed))
                    .unwrap_or_else(|| "Snapshot done".into());
                ctx.flash(msg);
                return;
            }
            Ok(st) if st.state.eq_ignore_ascii_case("failed") => {
                let msg = st
                    .last
                    .and_then(|l| l.error)
                    .unwrap_or_else(|| "Snapshot failed".into());
                ctx.flash(msg);
                return;
            }
            Ok(_) => {
                ctx.refresh.bump();
                ctx.flash("Snapshot finished");
                return;
            }
            Err(e) => {
                ctx.flash(e);
                return;
            }
        }
    }
    ctx.flash("Snapshot still running — check later");
}

async fn run_reload(ctx: CommandModeCtx) {
    flash_api_side_effect(
        ctx,
        fetch_settings(SettingsScope::Local).await,
        |v| {
            apply_theme_css_body(&v.css);
            ctx.refresh.bump();
        },
        "Reloaded config",
    )
    .await;
}

async fn run_export_zahir(ctx: CommandModeCtx) {
    flash_api(ctx, post_export_zahir().await, |out| {
        if out.count == 0 {
            "No Zahir JSON to export — retake snapshot after enhance".into()
        } else {
            format!("Exported {} Zahir JSON file(s)", out.count)
        }
    })
    .await;
}

async fn run_export_lenses(ctx: CommandModeCtx) {
    flash_api(ctx, post_export_lenses().await, |out| {
        if out.count == 0 {
            "No lenses to export — create a lens first".into()
        } else {
            format!("Exported {} lens Markdown file(s)", out.count)
        }
    })
    .await;
}

async fn open_theme_picker(ctx: CommandModeCtx) {
    let scope = ctx.theme_scope.get_untracked();
    match fetch_settings(scope).await {
        Ok(v) => {
            let restore = v.css.clone();
            let rows = theme_picker_rows(&v);
            let selected = crate::api::ThemePickerRow::index_of_theme(&rows, &v.theme);
            ctx.picker.set(Some(Picker::Theme {
                rows,
                selected,
                restore,
            }));
            apply_theme_preview(ctx);
        }
        Err(e) => ctx.flash(e),
    }
}

async fn open_root_picker(ctx: CommandModeCtx) {
    match fetch_roots().await {
        Ok(rows) => {
            if rows.is_empty() {
                ctx.flash("No indexed roots");
                return;
            }
            let selected = rows.iter().position(|r| r.current).unwrap_or(0);
            let paths: Vec<String> = rows.into_iter().map(|r| r.path).collect();
            ctx.picker.set(Some(Picker::Root { paths, selected }));
        }
        Err(e) => ctx.flash(e),
    }
}

pub(super) fn apply_theme_preview(ctx: CommandModeCtx) {
    let Some(p) = ctx.picker.get_untracked() else {
        return;
    };
    let Some(css) = p.selected_theme_css() else {
        return;
    };
    apply_theme_css_body(css);
}

/// Undo live preview when leaving the theme picker without committing.
pub(super) fn restore_theme_preview(ctx: CommandModeCtx) {
    let Some(Picker::Theme { restore, .. }) = ctx.picker.get_untracked() else {
        return;
    };
    apply_theme_css_body(&restore);
}

pub(super) fn submit_picker(ctx: CommandModeCtx) {
    let Some(p) = ctx.picker.get_untracked() else {
        return;
    };
    match p {
        Picker::Theme { .. } => {
            let Some(name) = p.selected_theme_name().map(str::to_string) else {
                return;
            };
            // Keep the highlighted preview; do not restore on dismiss.
            if let Some(css) = p.selected_theme_css() {
                apply_theme_css_body(css);
            }
            let restore = match &p {
                Picker::Theme { restore, .. } => restore.clone(),
                Picker::Root { .. } => crate::api::ThemeCssBody::default(),
            };
            ctx.picker.set(None);
            let scope = ctx.theme_scope.get_untracked();
            spawn_local(async move {
                let patch = SettingsPatch {
                    theme: Some(name.clone()),
                    ..Default::default()
                };
                match patch_settings(scope, &patch).await {
                    Ok(v) => {
                        apply_theme_css_body(&v.css);
                        ctx.theme_committed.update(|n| *n = n.wrapping_add(1));
                        ctx.flash(format!("Theme: {name}"));
                    }
                    Err(e) => {
                        apply_theme_css_body(&restore);
                        ctx.flash(e);
                    }
                }
            });
        }
        Picker::Root { paths, selected } => {
            let Some(dir) = paths.get(selected).cloned() else {
                return;
            };
            ctx.picker.set(None);
            spawn_local(async move {
                match switch_root(&dir).await {
                    Ok(cur) => {
                        ctx.flash(format!("Switched to {}", cur.path));
                        if let Some(w) = web_sys::window() {
                            let _ = w.location().reload();
                        }
                    }
                    Err(e) => ctx.flash(e),
                }
            });
        }
    }
}

pub(super) fn move_picker(ctx: CommandModeCtx, delta: i32) {
    ctx.picker.update(|p| {
        let Some(picker) = p.as_mut() else {
            return;
        };
        let n = picker.theme_count() as i32;
        if n == 0 {
            return;
        }
        match picker {
            Picker::Theme { selected, .. } | Picker::Root { selected, .. } => {
                *selected = ((*selected as i32 + delta).rem_euclid(n)) as usize;
            }
        }
    });
    apply_theme_preview(ctx);
}
