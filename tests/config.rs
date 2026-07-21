//! Config overlays: layout TOML, merge, and enhance (zahir batch) policy.

use ublx::config::{
    EnhancePolicy, EnhancePolicyEntry, LayoutOverlay, Osc11BackgroundFormat, UblxOpts, UblxOverlay,
};
use ublx::integrations::{NefaxOpts, ZahirRC};

#[test]
fn layout_overlay_default() {
    let layout = LayoutOverlay::default();
    assert_eq!(layout.left_pct, 10);
    assert_eq!(layout.middle_pct, 30);
    assert_eq!(layout.right_pct, 60);
}

#[test]
fn layout_overlay_parse_toml() {
    let toml = r#"
[layout]
left_pct = 25
middle_pct = 35
right_pct = 40
"#;
    let overlay: UblxOverlay = toml::from_str(toml).unwrap();
    let layout = overlay.layout.unwrap();
    assert_eq!(layout.left_pct, 25);
    assert_eq!(layout.middle_pct, 35);
    assert_eq!(layout.right_pct, 40);
}

#[test]
fn ublx_overlay_merge_local_overrides_global_run_snapshot_on_startup() {
    let global = UblxOverlay {
        run_snapshot_on_startup: Some(false),
        ..Default::default()
    };
    let local = UblxOverlay {
        run_snapshot_on_startup: Some(true),
        ..Default::default()
    };
    let m = UblxOverlay::merge(Some(global), Some(local));
    assert_eq!(m.run_snapshot_on_startup, Some(true));
}

#[test]
fn ublx_overlay_merge_global_run_snapshot_when_no_local() {
    let global = UblxOverlay {
        run_snapshot_on_startup: Some(false),
        ..Default::default()
    };
    let m = UblxOverlay::merge(Some(global), None);
    assert_eq!(m.run_snapshot_on_startup, Some(false));
}

#[test]
fn ublx_overlay_merge_local_does_not_override_global_only_keys() {
    use ublx::config::CommandModeOverlay;

    let global = UblxOverlay {
        opacity_format: Some(Osc11BackgroundFormat::Rgba),
        ask_enhance_on_new_root: Some(true),
        command_mode: Some(CommandModeOverlay {
            leader: Some("a".into()),
        }),
        ..Default::default()
    };
    let local = UblxOverlay {
        opacity_format: Some(Osc11BackgroundFormat::Hex8),
        ask_enhance_on_new_root: Some(false),
        command_mode: Some(CommandModeOverlay {
            leader: Some("b".into()),
        }),
        ..Default::default()
    };
    let m = UblxOverlay::merge(Some(global), Some(local));
    assert_eq!(m.opacity_format, Some(Osc11BackgroundFormat::Rgba));
    assert_eq!(m.ask_enhance_on_new_root, Some(true));
    assert_eq!(
        m.command_mode.as_ref().and_then(|c| c.leader.as_deref()),
        Some("a")
    );
}

#[test]
fn ublx_overlay_merge_layout() {
    let mut base = UblxOverlay::default();
    let other = UblxOverlay {
        layout: Some(LayoutOverlay {
            left_pct: 10,
            middle_pct: 45,
            right_pct: 45,
        }),
        ..Default::default()
    };
    base.merge_from(&other);
    let layout = base.layout.unwrap();
    assert_eq!(layout.left_pct, 10);
    assert_eq!(layout.middle_pct, 45);
    assert_eq!(layout.right_pct, 45);
}

fn opts_with(enable_enhance_all: bool, entries: Vec<EnhancePolicyEntry>) -> UblxOpts {
    UblxOpts {
        nefax_opts: NefaxOpts::default(),
        zahir_rc: ZahirRC::new(),
        max_workers_available: 1,
        nefax_workers_override: None,
        zahir_workers_override: None,
        ublx_workers_override: None,
        tokio_runtime_workers: 2,
        streaming: false,
        config_source: None,
        theme: None,
        layout: LayoutOverlay::default(),
        bg_opacity: None,
        opacity_format: Osc11BackgroundFormat::default(),
        editor_path: None,
        enable_enhance_all,
        ask_enhance_on_new_root: true,
        enable_enhance_all_cache_before_apply: None,
        with_hash_cache_before_apply: None,
        enhance_policy: entries,
        run_snapshot_on_startup: true,
        typed_column_tables: ublx::config::ColumnStatsDisplay::default(),
        command_mode_leader: ublx::config::DEFAULT_COMMAND_MODE_LEADER,
    }
}

#[test]
fn manual_overrides_global_on() {
    let o = opts_with(
        true,
        vec![EnhancePolicyEntry {
            path: "blocked".into(),
            policy: EnhancePolicy::Manual,
        }],
    );
    assert!(!o.batch_zahir_for_path("blocked/file.txt"));
    assert!(o.batch_zahir_for_path("other/file.txt"));
}

#[test]
fn auto_overrides_global_off() {
    let o = opts_with(
        false,
        vec![EnhancePolicyEntry {
            path: "force".into(),
            policy: EnhancePolicy::Auto,
        }],
    );
    assert!(o.batch_zahir_for_path("force/a.rs"));
    assert!(!o.batch_zahir_for_path("outside/a.rs"));
}

#[test]
fn longest_prefix_wins() {
    let o = opts_with(
        true,
        vec![
            EnhancePolicyEntry {
                path: "a".into(),
                policy: EnhancePolicy::Manual,
            },
            EnhancePolicyEntry {
                path: "a/b".into(),
                policy: EnhancePolicy::Auto,
            },
        ],
    );
    assert!(!o.batch_zahir_for_path("a/x"));
    assert!(o.batch_zahir_for_path("a/b/x"));
}

#[test]
fn deserializes_legacy_always_never_toml() {
    let s = r#"
        [[enhance_policy]]
        path = "legacy"
        policy = "always"
        [[enhance_policy]]
        path = "legacy2"
        policy = "never"
    "#;
    let overlay: UblxOverlay = toml::from_str(s).expect("parse");
    let entries = overlay.enhance_policy.expect("entries");
    assert_eq!(entries[0].policy, EnhancePolicy::Auto);
    assert_eq!(entries[1].policy, EnhancePolicy::Manual);
}

#[test]
fn typed_column_tables_overlay_merge_local_overrides_global() {
    use ublx::config::ColumnStatsDisplay;
    let global = UblxOverlay {
        typed_column_tables: Some(ColumnStatsDisplay::Full),
        ..Default::default()
    };
    let local = UblxOverlay {
        typed_column_tables: Some(ColumnStatsDisplay::None),
        ..Default::default()
    };
    let m = UblxOverlay::merge(Some(global), Some(local));
    assert_eq!(m.typed_column_tables, Some(ColumnStatsDisplay::None));
}

#[test]
fn typed_column_tables_overlay_parse_toml() {
    use ublx::config::ColumnStatsDisplay;
    let overlay: UblxOverlay = toml::from_str("typed_column_tables = \"abbrev\"\n").unwrap();
    assert_eq!(
        overlay.typed_column_tables,
        Some(ColumnStatsDisplay::Abbrev)
    );
}

#[test]
fn typed_column_tables_overlay_parse_legacy_column_stats_alias() {
    use ublx::config::ColumnStatsDisplay;
    let overlay: UblxOverlay = toml::from_str("column_stats = \"full\"\n").unwrap();
    assert_eq!(overlay.typed_column_tables, Some(ColumnStatsDisplay::Full));
}

#[test]
fn overlay_backfill_adds_typed_column_tables_when_missing() {
    use ublx::config::{ColumnStatsDisplay, default_overlay_for_new_file};
    let template = default_overlay_for_new_file("default");
    let mut existing = UblxOverlay {
        theme: Some("custom".into()),
        ..Default::default()
    };
    assert!(existing.backfill_missing_from_template(&template, false));
    assert_eq!(
        existing.typed_column_tables,
        Some(ColumnStatsDisplay::Abbrev)
    );
    assert_eq!(existing.theme.as_deref(), Some("custom"));
}

#[test]
fn overlay_backfill_does_not_overwrite_existing_typed_column_tables() {
    use ublx::config::{ColumnStatsDisplay, default_overlay_for_new_file};
    let template = default_overlay_for_new_file("default");
    let mut existing = UblxOverlay {
        typed_column_tables: Some(ColumnStatsDisplay::Full),
        ..Default::default()
    };
    existing.backfill_missing_from_template(&template, false);
    assert_eq!(existing.typed_column_tables, Some(ColumnStatsDisplay::Full));
}

#[test]
fn overlay_backfill_skips_global_only_keys_on_local_scope() {
    use ublx::config::default_overlay_for_new_file;
    let template = default_overlay_for_new_file("default");
    let mut existing = UblxOverlay::default();
    assert!(existing.backfill_missing_from_template(&template, true));
    assert!(existing.ask_enhance_on_new_root.is_none());
    assert!(existing.opacity_format.is_none());
    assert_eq!(
        existing.typed_column_tables,
        Some(ublx::config::ColumnStatsDisplay::Abbrev)
    );
}

#[test]
fn ensure_global_config_backfills_existing_file_on_disk() {
    use std::fs;
    use ublx::config::{
        ColumnStatsDisplay, ensure_global_config_file_with_defaults, load_ublx_toml,
    };
    let path = std::env::temp_dir().join(format!(
        "ublx-global-config-test-{}-column-stats",
        std::process::id()
    ));
    let _cleanup = TempConfigCleanup(path.clone());
    fs::write(&path, "theme = \"custom\"\n").unwrap();
    assert!(ensure_global_config_file_with_defaults(&path, "default"));
    let overlay = load_ublx_toml(Some(path), None).unwrap();
    assert_eq!(overlay.theme.as_deref(), Some("custom"));
    assert_eq!(
        overlay.typed_column_tables,
        Some(ColumnStatsDisplay::Abbrev)
    );
}

#[test]
fn settings_typed_column_tables_row_layout_indices() {
    use ublx::layout::setup::SettingsConfigScope;
    use ublx::modules::settings::{
        bool_row_count, command_mode_leader_row_index, layout_button_index,
        opacity_format_row_index, typed_column_tables_row_index,
    };

    assert_eq!(
        typed_column_tables_row_index(SettingsConfigScope::Global),
        5
    );
    assert_eq!(typed_column_tables_row_index(SettingsConfigScope::Local), 4);
    assert_eq!(
        command_mode_leader_row_index(SettingsConfigScope::Global),
        Some(6)
    );
    assert_eq!(
        command_mode_leader_row_index(SettingsConfigScope::Local),
        None
    );
    assert_eq!(
        opacity_format_row_index(SettingsConfigScope::Global),
        Some(7)
    );
    assert_eq!(opacity_format_row_index(SettingsConfigScope::Local), None);
    assert_eq!(layout_button_index(SettingsConfigScope::Global), 8);
    assert_eq!(layout_button_index(SettingsConfigScope::Local), 5);
    assert_eq!(bool_row_count(SettingsConfigScope::Global), 5);
    assert_eq!(bool_row_count(SettingsConfigScope::Local), 4);
}

#[test]
fn typed_column_tables_cycle_order() {
    use ublx::config::ColumnStatsDisplay;
    use ublx::modules::settings::cycle_typed_column_tables;

    assert_eq!(
        cycle_typed_column_tables(ColumnStatsDisplay::None),
        ColumnStatsDisplay::Abbrev
    );
    assert_eq!(
        cycle_typed_column_tables(ColumnStatsDisplay::Abbrev),
        ColumnStatsDisplay::Full
    );
    assert_eq!(
        cycle_typed_column_tables(ColumnStatsDisplay::Full),
        ColumnStatsDisplay::None
    );
}

#[test]
fn command_mode_leader_parse_and_cycle() {
    use ublx::config::{
        CommandModeOverlay, UblxOverlay, cycle_command_mode_leader, parse_command_mode_leader,
        validate_hot_reload_overlay,
    };

    assert_eq!(parse_command_mode_leader("A").unwrap(), 'a');
    assert!(parse_command_mode_leader("j").is_err());
    assert!(parse_command_mode_leader("k").is_err());
    assert!(parse_command_mode_leader("ab").is_err());
    assert_eq!(cycle_command_mode_leader('a'), 'b');
    assert_eq!(cycle_command_mode_leader('i'), 'l'); // skips j/k
    assert_eq!(cycle_command_mode_leader('z'), 'a');

    let bad = UblxOverlay {
        command_mode: Some(CommandModeOverlay {
            leader: Some("j".into()),
        }),
        ..Default::default()
    };
    let err = validate_hot_reload_overlay(&bad, &[]).unwrap_err();
    assert!(err.iter().any(|e| e.field == "command_mode.leader"));
}

#[test]
fn command_mode_overlay_parse_toml() {
    let toml = r#"
[command_mode]
leader = "b"
"#;
    let overlay: UblxOverlay = toml::from_str(toml).unwrap();
    assert_eq!(
        overlay
            .command_mode
            .as_ref()
            .and_then(|c| c.leader.as_deref()),
        Some("b")
    );
}

struct TempConfigCleanup(std::path::PathBuf);

impl Drop for TempConfigCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
