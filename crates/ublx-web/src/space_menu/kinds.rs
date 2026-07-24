//! Menu kind / pending sub-flow / row labels (TUI `qa_menu_item_labels` subset).

/// Which Space menu to open (file / lens / duplicate / bulk).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpaceMenuKind {
    File {
        path: String,
        lenses: bool,
        /// Show **Open in new tab** (images only).
        open_in_tab: bool,
    },
    Lens {
        name: String,
    },
    Duplicate {
        path: String,
        open_in_tab: bool,
    },
    Bulk {
        lenses: bool,
    },
}

/// Sub-flow after a menu row (rename / confirm / lens picker / …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Pending {
    Rename {
        /// Catalog path or lens name.
        target: String,
        draft: String,
        lens: bool,
    },
    DeleteConfirm {
        paths: Vec<String>,
        /// Lens name delete (panel) vs file paths.
        lens_name: Option<String>,
    },
    LensPicker {
        paths: Vec<String>,
        names: Vec<String>,
        exclude: Option<String>,
    },
    NewLens {
        paths: Vec<String>,
        draft: String,
    },
    EnhancePolicy {
        path: String,
    },
    BulkRename {
        paths: Vec<String>,
        draft: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MenuRow {
    pub(super) label: &'static str,
    pub(super) key: char,
}

const fn menu_row(label: &'static str, key: char) -> MenuRow {
    MenuRow { label, key }
}

pub(super) fn pending_title(pending: Option<&Pending>, kind: Option<&SpaceMenuKind>) -> String {
    match pending {
        Some(Pending::DeleteConfirm { .. }) => " Confirm delete ".into(),
        Some(Pending::EnhancePolicy { .. }) => " Enhance policy ".into(),
        Some(Pending::LensPicker { .. }) => " Add to Lens ".into(),
        _ => kind
            .map(title_for)
            .map(|t| format!(" {t} "))
            .unwrap_or_else(|| " Actions ".into()),
    }
}

pub(super) fn pending_allows_navigation(pending: Option<&Pending>) -> bool {
    matches!(
        pending,
        None | Some(
            Pending::DeleteConfirm { .. }
                | Pending::EnhancePolicy { .. }
                | Pending::LensPicker { .. }
        )
    )
}

pub(super) fn menu_rows_for_kind(kind: Option<&SpaceMenuKind>) -> Vec<MenuRow> {
    match kind {
        Some(k) => menu_rows(k),
        None => Vec::new(),
    }
}

pub(super) fn menu_rows(kind: &SpaceMenuKind) -> Vec<MenuRow> {
    match kind {
        SpaceMenuKind::File {
            lenses,
            open_in_tab,
            ..
        } => {
            let mut v = Vec::new();
            if *open_in_tab {
                v.push(menu_row("Open in new tab", 'o'));
            }
            if !*lenses {
                v.push(menu_row("Enhance policy", 'p'));
            }
            v.push(menu_row("Enhance with ZahirScan", 'z'));
            if *lenses {
                v.push(menu_row("Delete from current Lens", 'd'));
            } else {
                v.push(menu_row("Add to Lens", 'l'));
            }
            v.extend([
                menu_row("Copy Path", 'c'),
                menu_row("Copy Zahir JSON", 'j'),
                menu_row("Rename", 'r'),
            ]);
            if !*lenses {
                v.push(menu_row("Delete", 'd'));
            }
            v
        }
        SpaceMenuKind::Lens { .. } => vec![menu_row("Rename", 'r'), menu_row("Delete", 'd')],
        SpaceMenuKind::Duplicate { open_in_tab, .. } => {
            let mut v = vec![
                menu_row("Delete", 'd'),
                menu_row("Ignore", 'i'),
                menu_row("Copy Path", 'c'),
            ];
            if *open_in_tab {
                v.insert(2, menu_row("Open in new tab", 'o'));
            }
            v
        }
        SpaceMenuKind::Bulk { lenses } => vec![
            menu_row(
                if *lenses {
                    "Add to other lens"
                } else {
                    "Add to Lens"
                },
                'a',
            ),
            menu_row("Rename", 'r'),
            menu_row(
                if *lenses {
                    "Delete from current Lens"
                } else {
                    "Delete"
                },
                'd',
            ),
            menu_row("Enhance with ZahirScan", 'z'),
        ],
    }
}

pub(super) fn title_for(kind: &SpaceMenuKind) -> &'static str {
    match kind {
        SpaceMenuKind::File { .. } => "Actions",
        SpaceMenuKind::Lens { .. } => "Lens",
        SpaceMenuKind::Duplicate { .. } => "Duplicates",
        SpaceMenuKind::Bulk { .. } => "Bulk",
    }
}

pub(super) fn label_with_hotkey(label: &str, key: char) -> String {
    format!("{label} ({key})")
}
