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
    let global = UblxOverlay {
        opacity_format: Some(Osc11BackgroundFormat::Rgba),
        ask_enhance_on_new_root: Some(true),
        ..Default::default()
    };
    let local = UblxOverlay {
        opacity_format: Some(Osc11BackgroundFormat::Hex8),
        ask_enhance_on_new_root: Some(false),
        ..Default::default()
    };
    let m = UblxOverlay::merge(Some(global), Some(local));
    assert_eq!(m.opacity_format, Some(Osc11BackgroundFormat::Rgba));
    assert_eq!(m.ask_enhance_on_new_root, Some(true));
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
        column_stats: ublx::config::ColumnStatsDisplay::default(),
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
fn column_stats_overlay_merge_local_overrides_global() {
    use ublx::config::ColumnStatsDisplay;
    let global = UblxOverlay {
        column_stats: Some(ColumnStatsDisplay::Full),
        ..Default::default()
    };
    let local = UblxOverlay {
        column_stats: Some(ColumnStatsDisplay::None),
        ..Default::default()
    };
    let m = UblxOverlay::merge(Some(global), Some(local));
    assert_eq!(m.column_stats, Some(ColumnStatsDisplay::None));
}

#[test]
fn column_stats_overlay_parse_toml() {
    use ublx::config::ColumnStatsDisplay;
    let overlay: UblxOverlay = toml::from_str("column_stats = \"abbrev\"\n").unwrap();
    assert_eq!(overlay.column_stats, Some(ColumnStatsDisplay::Abbrev));
}
