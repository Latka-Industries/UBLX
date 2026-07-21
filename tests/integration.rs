//! Subprocess and full-frame smoke tests (binary + render pipeline without a real terminal).

use std::path::Path;
use std::process::Command;
use ublx::config::{DEFAULT_COMMAND_MODE_LEADER, UblxPaths, last_applied_config_path};
use ublx::engine::db_ops::DuplicateGroupingMode;

fn ublx_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ublx"))
}

fn remove_if_exists(path_ref: &Path) {
    if path_ref.exists() {
        let _ = std::fs::remove_file(path_ref);
    }
}

/// Remove cache artifacts for the deterministic integration dir:
/// per-root DB files and per-root cached applied config.
fn cleanup_integration_test_cache(dir_ref: &Path) {
    let paths = UblxPaths::new(dir_ref);
    remove_if_exists(&paths.db());
    remove_if_exists(&paths.wal());
    remove_if_exists(&paths.shm());
    remove_if_exists(&paths.tmp());
    remove_if_exists(&paths.tmp_wal());
    remove_if_exists(&paths.tmp_shm());
    remove_if_exists(&paths.hidden_toml());
    remove_if_exists(&paths.visible_toml());
    if let Some(cfg) = last_applied_config_path(dir_ref) {
        remove_if_exists(&cfg);
    }
}

#[test]
fn help_exits_zero() {
    let out = ublx_bin().arg("--help").output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ublx") || stdout.contains("DIR"),
        "stdout: {}",
        stdout
    );
}

#[test]
fn snapshot_only_writes_db_and_hidden_toml() {
    let tmp = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("ublx_integration_test_dir");
    let _ = std::fs::create_dir_all(&tmp);
    // Must match `validate_dir` → `canonicalize()` in main: `UblxPaths::db_stem` hashes this path.
    let tmp = tmp
        .canonicalize()
        .expect("canonicalize integration test dir");
    cleanup_integration_test_cache(&tmp);
    let paths = UblxPaths::new(&tmp);
    let out = ublx_bin()
        .arg("--snapshot-only")
        .arg(&tmp)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "exit: {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        paths.db().exists(),
        "expected ubli db after --snapshot-only"
    );
    assert!(
        paths.hidden_toml().exists(),
        "expected .ublx.toml after --snapshot-only in fresh dir"
    );
    let toml = std::fs::read_to_string(paths.hidden_toml()).unwrap();
    assert!(
        toml.contains("enable_enhance_all = false"),
        "expected enable_enhance_all = false, got: {toml}"
    );
    cleanup_integration_test_cache(&tmp);
}

#[test]
fn draw_one_snapshot_frame_renders_main_chrome() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ublx::config::LayoutOverlay;
    use ublx::layout::setup::{RightPaneContent, UblxState, ViewContents, ViewData};
    use ublx::render::{DrawFrameArgs, draw_ublx_frame};

    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");

    let mut state = UblxState::new();
    let view = ViewData {
        filtered_categories: vec!["misc".to_string()],
        contents: ViewContents::SnapshotIndices(vec![]),
        category_list_len: 1,
        content_len: 0,
    };
    let right = RightPaneContent::empty();
    let layout = LayoutOverlay::default();
    let args = DrawFrameArgs {
        bg_opacity: 1.0,
        transparent_page_chrome: false,
        delta_data: None,
        all_rows: None,
        dir_to_ublx: None,
        theme_name: None,
        layout: &layout,
        last_snapshot_ns: None,
        dev: false,
        duplicate_groups: None,
        duplicate_mode: DuplicateGroupingMode::Hash,
        lens_names: None,
        command_mode_leader: DEFAULT_COMMAND_MODE_LEADER,
    };

    terminal
        .draw(|f| {
            draw_ublx_frame(f, &mut state, &view, &right, &args);
        })
        .expect("draw one frame");

    let buf: &Buffer = terminal.backend().buffer();
    let flat: String = buf.content().iter().map(|c| c.symbol()).collect();

    assert!(
        flat.contains("UBLX") || flat.contains("Snapshot"),
        "expected tab row / brand in buffer; sample: {}",
        flat.chars().take(400).collect::<String>()
    );
}
