//! `SpaceMenuCtx` — open / submit / apply mutations (keyboard-safe: no `expect_context` in hot path).

use std::collections::HashSet;
use std::future::Future;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api::{
    BulkRenameItem, api_add_to_lens, api_bulk_rename, api_create_lens, api_delete, api_delete_lens,
    api_enhance, api_enhance_policy, api_remove_from_lens, api_rename, api_rename_lens,
    fetch_entry_zahir_raw, fetch_lens_names,
};
use crate::catalog_refresh::CatalogRefresh;
use crate::focus::PaneFocus;
use crate::multiselect::MultiselectCtx;
use crate::nav::MainMode;
use crate::toast::ToastCtx;

use super::helpers::{absolute_path, basename, bulk_paths, open_in_browser, write_clipboard};
use super::kinds::{
    MenuRow, Pending, SpaceMenuKind, label_with_hotkey, menu_rows, menu_rows_for_kind,
    pending_allows_navigation,
};

#[derive(Clone, Copy)]
pub(crate) struct SpaceMenuCtx {
    pub visible: RwSignal<bool>,
    pub kind: RwSignal<Option<SpaceMenuKind>>,
    pub selected: RwSignal<usize>,
    pub middle_path: RwSignal<Option<String>>,
    pub left_label: RwSignal<Option<String>>,
    pub ignored: RwSignal<HashSet<String>>,
    /// Catalog root for Copy Path (set from Shell).
    pub catalog_root: RwSignal<Option<String>>,
    pub(super) pending: RwSignal<Option<Pending>>,
    /// Stored handles — keyboard path runs outside Leptos owner (`expect_context` would panic).
    refresh: CatalogRefresh,
    multiselect: MultiselectCtx,
    toasts: ToastCtx,
}

impl SpaceMenuCtx {
    pub(crate) fn provide(
        refresh: CatalogRefresh,
        multiselect: MultiselectCtx,
        toasts: ToastCtx,
    ) -> Self {
        let ctx = Self {
            visible: RwSignal::new(false),
            kind: RwSignal::new(None),
            selected: RwSignal::new(0),
            middle_path: RwSignal::new(None),
            left_label: RwSignal::new(None),
            ignored: RwSignal::new(HashSet::new()),
            catalog_root: RwSignal::new(None),
            pending: RwSignal::new(None),
            refresh,
            multiselect,
            toasts,
        };
        provide_context(ctx);
        ctx
    }

    pub(crate) fn expect() -> Self {
        expect_context::<Self>()
    }

    pub(crate) fn close(self) {
        self.visible.set(false);
        self.kind.set(None);
        self.selected.set(0);
        self.pending.set(None);
    }

    pub(crate) fn flash(self, msg: impl Into<String>) {
        self.toasts.info(msg);
    }

    pub(crate) fn flash_warn(self, msg: impl Into<String>) {
        self.toasts.warn(msg);
    }

    pub(crate) fn flash_err(self, msg: impl Into<String>) {
        self.toasts.error(msg);
    }

    pub(crate) fn open(self, kind: SpaceMenuKind) {
        self.pending.set(None);
        self.kind.set(Some(kind));
        self.selected.set(0);
        self.visible.set(true);
    }

    pub(crate) fn try_open_qa(
        self,
        mode: MainMode,
        pane: PaneFocus,
        multiselect_active: bool,
    ) -> bool {
        if multiselect_active || self.visible.get_untracked() {
            return false;
        }
        match mode {
            MainMode::Delta | MainMode::Settings => return false,
            MainMode::Snapshot | MainMode::Lenses | MainMode::Duplicates => {}
        }

        if pane == PaneFocus::Middle {
            let Some(path) = self.middle_path.get_untracked() else {
                return false;
            };
            let kind = match mode {
                MainMode::Duplicates => SpaceMenuKind::Duplicate { path },
                MainMode::Lenses => SpaceMenuKind::File { path, lenses: true },
                _ => SpaceMenuKind::File {
                    path,
                    lenses: false,
                },
            };
            self.open(kind);
            return true;
        }

        if mode == MainMode::Lenses
            && pane == PaneFocus::Left
            && let Some(name) = self.left_label.get_untracked()
        {
            self.open(SpaceMenuKind::Lens { name });
            return true;
        }
        false
    }

    pub(crate) fn try_open_bulk(self, mode: MainMode, multiselect_active: bool) -> bool {
        if !multiselect_active || self.visible.get_untracked() {
            return false;
        }
        let lenses = match mode {
            MainMode::Lenses => true,
            MainMode::Snapshot => false,
            _ => return false,
        };
        self.open(SpaceMenuKind::Bulk { lenses });
        true
    }

    pub(crate) fn move_sel(self, delta: i32) {
        if !pending_allows_navigation(self.pending.get_untracked().as_ref()) {
            return;
        }
        let n = self.display_rows().len() as i32;
        if n == 0 {
            return;
        }
        let cur = self.selected.get_untracked() as i32;
        let next = (cur + delta).clamp(0, n - 1) as usize;
        self.selected.set(next);
    }

    pub(super) fn display_rows(self) -> Vec<(String, char)> {
        match self.pending.get_untracked() {
            Some(Pending::DeleteConfirm { .. }) => {
                vec![("Yes — delete".into(), 'y'), ("No".into(), 'n')]
            }
            Some(Pending::EnhancePolicy { .. }) => vec![
                ("Always — automatic".into(), 'y'),
                ("Per-file — manual".into(), 'n'),
            ],
            Some(Pending::LensPicker { names, .. }) => {
                let mut rows = vec![("＋ New lens…".into(), 'n')];
                for (i, name) in names.iter().enumerate() {
                    let key = if i < 9 {
                        char::from_digit((i + 1) as u32, 10).unwrap_or(' ')
                    } else {
                        ' '
                    };
                    rows.push((name.clone(), key));
                }
                rows
            }
            Some(Pending::Rename { .. } | Pending::NewLens { .. } | Pending::BulkRename { .. }) => {
                Vec::new()
            }
            None => match self.kind.get_untracked() {
                Some(ref k) => menu_rows(k)
                    .into_iter()
                    .map(|r| (label_with_hotkey(r.label, r.key), r.key))
                    .collect(),
                None => Vec::new(),
            },
        }
    }

    pub(crate) fn submit_selected(self, root: Option<String>) {
        let root = root.or_else(|| self.catalog_root.get_untracked());
        self.submit_row_at(self.selected.get_untracked(), root);
    }

    /// Mouse / explicit index activate (select row then submit).
    pub(crate) fn activate_index(self, idx: usize) {
        self.submit_row_at(idx, self.catalog_root.get_untracked());
    }

    fn submit_row_at(self, idx: usize, root: Option<String>) {
        self.selected.set(idx);
        if let Some(p) = self.pending.get_untracked() {
            self.submit_pending(p, root);
            return;
        }
        let rows = menu_rows_for_kind(self.kind.get_untracked().as_ref());
        if let Some(row) = rows.get(idx).copied() {
            self.apply_row(row, root);
        }
    }

    pub(crate) fn submit_hotkey(self, key: char, root: Option<String>) -> bool {
        let root = root.or_else(|| self.catalog_root.get_untracked());
        let c = key.to_ascii_lowercase();
        if let Some(p) = self.pending.get_untracked() {
            match p {
                Pending::DeleteConfirm { .. } | Pending::EnhancePolicy { .. } => {
                    if c == 'y' {
                        self.selected.set(0);
                        self.submit_pending(p, root);
                        return true;
                    }
                    if c == 'n' {
                        self.selected.set(1);
                        self.submit_pending(p, root);
                        return true;
                    }
                }
                Pending::LensPicker { .. } if c == 'n' => {
                    self.selected.set(0);
                    self.submit_pending(p, root);
                    return true;
                }
                Pending::LensPicker { ref names, .. } if c.is_ascii_digit() && c != '0' => {
                    let n = c.to_digit(10).unwrap_or(0) as usize;
                    // Row 0 = new lens; digits 1–9 map to names[0..].
                    if n >= 1 && n <= names.len().min(9) {
                        self.selected.set(n);
                        self.submit_pending(p, root);
                        return true;
                    }
                }
                _ => {}
            }
            return false;
        }
        let rows = menu_rows_for_kind(self.kind.get_untracked().as_ref());
        let Some(idx) = rows.iter().position(|r| r.key == c) else {
            return false;
        };
        self.selected.set(idx);
        if let Some(row) = rows.get(idx).copied() {
            self.apply_row(row, root);
        }
        true
    }

    fn submit_pending(self, pending: Pending, _root: Option<String>) {
        let refresh = self.refresh;
        let ms = self.multiselect;
        match pending {
            Pending::DeleteConfirm { paths, lens_name } => {
                let idx = self.selected.get_untracked();
                if idx != 0 {
                    self.pending.set(None);
                    self.selected.set(0);
                    return;
                }
                self.close();
                if let Some(name) = lens_name {
                    spawn_api(
                        self,
                        refresh,
                        ms,
                        ApiEffects {
                            clear: false,
                            bump: true,
                        },
                        async move {
                            api_delete_lens(&name)
                                .await
                                .map(|()| format!("Deleted lens {name}"))
                        },
                    );
                } else {
                    spawn_api(
                        self,
                        refresh,
                        ms,
                        ApiEffects {
                            clear: true,
                            bump: true,
                        },
                        api_delete(paths),
                    );
                }
            }
            Pending::EnhancePolicy { path } => {
                let idx = self.selected.get_untracked();
                let policy = if idx == 0 { "auto" } else { "manual" };
                self.close();
                let path = path.clone();
                spawn_api(self, refresh, ms, ApiEffects::flash_only(), async move {
                    api_enhance_policy(&path, policy).await
                });
            }
            Pending::LensPicker {
                paths,
                names,
                exclude: _,
            } => {
                let idx = self.selected.get_untracked();
                if idx == 0 {
                    self.pending.set(Some(Pending::NewLens {
                        paths,
                        draft: String::new(),
                    }));
                    return;
                }
                let Some(name) = names.get(idx - 1).cloned() else {
                    return;
                };
                self.close();
                spawn_api(
                    self,
                    refresh,
                    ms,
                    ApiEffects {
                        clear: true,
                        bump: true,
                    },
                    async move { api_add_to_lens(&name, paths).await },
                );
            }
            Pending::Rename { .. } | Pending::NewLens { .. } | Pending::BulkRename { .. } => {
                // Submitted via input Enter handlers.
            }
        }
    }

    fn apply_row(self, row: MenuRow, root: Option<String>) {
        let kind = self.kind.get_untracked();
        let Some(kind) = kind else {
            return;
        };
        let refresh = self.refresh;
        let ms = self.multiselect;

        match (&kind, row.key) {
            (SpaceMenuKind::File { path, .. } | SpaceMenuKind::Duplicate { path }, 'o') => {
                self.close();
                open_in_browser(path);
                self.flash("Opened in new tab");
            }
            (SpaceMenuKind::File { path, .. } | SpaceMenuKind::Duplicate { path }, 'c') => {
                self.copy_abs_path(root, path, "Copied path");
            }
            (SpaceMenuKind::File { path, .. }, 'j') => {
                self.close();
                let path = path.clone();
                spawn_local(async move {
                    copy_zahir_json(self, &path).await;
                });
            }
            (
                SpaceMenuKind::File {
                    path,
                    lenses: false,
                },
                'p',
            ) => {
                self.pending
                    .set(Some(Pending::EnhancePolicy { path: path.clone() }));
                self.selected.set(0);
            }
            (SpaceMenuKind::File { path, .. }, 'z') => {
                self.close();
                let path = path.clone();
                spawn_api(
                    self,
                    refresh,
                    ms,
                    ApiEffects {
                        clear: false,
                        bump: true,
                    },
                    api_enhance(vec![path]),
                );
            }
            (
                SpaceMenuKind::File {
                    path,
                    lenses: false,
                },
                'l',
            ) => {
                let path = path.clone();
                spawn_local(async move {
                    open_lens_picker(self, vec![path], None).await;
                });
            }
            (SpaceMenuKind::File { path, lenses: true }, 'd') => {
                self.close();
                let path = path.clone();
                let lens = self.left_label.get_untracked().unwrap_or_default();
                spawn_api(
                    self,
                    refresh,
                    ms,
                    ApiEffects {
                        clear: false,
                        bump: true,
                    },
                    async move { api_remove_from_lens(&lens, vec![path]).await },
                );
            }
            (SpaceMenuKind::File { path, .. }, 'r') => {
                let base = basename(path);
                self.pending.set(Some(Pending::Rename {
                    target: path.clone(),
                    draft: base,
                    lens: false,
                }));
            }
            (
                SpaceMenuKind::File {
                    path,
                    lenses: false,
                },
                'd',
            )
            | (SpaceMenuKind::Duplicate { path }, 'd') => {
                self.confirm_delete(vec![path.clone()], None);
            }
            (SpaceMenuKind::Duplicate { path }, 'i') => {
                self.close();
                self.ignored.update(|s| {
                    s.insert(path.clone());
                });
                self.flash("Ignored for this session");
            }
            (SpaceMenuKind::Lens { name }, 'r') => {
                self.pending.set(Some(Pending::Rename {
                    target: name.clone(),
                    draft: name.clone(),
                    lens: true,
                }));
            }
            (SpaceMenuKind::Lens { name }, 'd') => {
                self.confirm_delete(Vec::new(), Some(name.clone()));
            }
            (SpaceMenuKind::Bulk { lenses }, 'a') => {
                let Some(paths) = self.bulk_paths_or_flash(ms) else {
                    return;
                };
                let exclude = if *lenses {
                    self.left_label.get_untracked()
                } else {
                    None
                };
                spawn_local(async move {
                    open_lens_picker(self, paths, exclude).await;
                });
            }
            (SpaceMenuKind::Bulk { .. }, 'r') => {
                let Some(paths) = self.bulk_paths_or_flash(ms) else {
                    return;
                };
                let draft = paths
                    .iter()
                    .map(|p| basename(p))
                    .collect::<Vec<_>>()
                    .join("\n");
                self.pending.set(Some(Pending::BulkRename { paths, draft }));
            }
            (SpaceMenuKind::Bulk { lenses }, 'd') => {
                let Some(paths) = self.bulk_paths_or_flash(ms) else {
                    return;
                };
                if *lenses {
                    self.close();
                    let lens = self.left_label.get_untracked().unwrap_or_default();
                    spawn_api(
                        self,
                        refresh,
                        ms,
                        ApiEffects {
                            clear: true,
                            bump: true,
                        },
                        async move { api_remove_from_lens(&lens, paths).await },
                    );
                } else {
                    self.confirm_delete(paths, None);
                }
            }
            (SpaceMenuKind::Bulk { .. }, 'z') => {
                self.close();
                let paths = bulk_paths(ms);
                spawn_api(
                    self,
                    refresh,
                    ms,
                    ApiEffects {
                        clear: true,
                        bump: true,
                    },
                    api_enhance(paths),
                );
            }
            _ => {
                self.close();
                self.flash_warn(format!("{} — not wired", row.label));
            }
        }
    }

    pub(crate) fn commit_rename_draft(self) {
        let Some(Pending::Rename {
            target,
            draft,
            lens,
        }) = self.pending.get_untracked()
        else {
            return;
        };
        let name = draft.trim().to_string();
        if name.is_empty() {
            self.flash_warn("name is empty");
            return;
        }
        self.close();
        let refresh = self.refresh;
        spawn_api(
            self,
            refresh,
            self.multiselect,
            ApiEffects {
                clear: false,
                bump: true,
            },
            async move {
                if lens {
                    api_rename_lens(&target, &name).await
                } else {
                    api_rename(&target, &name)
                        .await
                        .map(|p| format!("Renamed → {p}"))
                }
            },
        );
    }

    pub(crate) fn commit_new_lens(self) {
        let Some(Pending::NewLens { paths, draft }) = self.pending.get_untracked() else {
            return;
        };
        let name = draft.trim().to_string();
        if name.is_empty() {
            self.flash_warn("lens name is empty");
            return;
        }
        self.close();
        spawn_api(
            self,
            self.refresh,
            self.multiselect,
            ApiEffects {
                clear: true,
                bump: true,
            },
            async move { api_create_lens(&name, paths).await },
        );
    }

    pub(crate) fn commit_bulk_rename(self) {
        let Some(Pending::BulkRename { paths, draft }) = self.pending.get_untracked() else {
            return;
        };
        let lines: Vec<&str> = draft.lines().map(str::trim).collect();
        if lines.len() != paths.len() || lines.iter().any(|l| l.is_empty()) {
            self.flash_warn(format!(
                "need {} non-empty basename lines (got {})",
                paths.len(),
                lines.len()
            ));
            return;
        }
        let renames: Vec<BulkRenameItem> = paths
            .into_iter()
            .zip(lines)
            .map(|(path, new_name)| BulkRenameItem {
                path,
                new_name: new_name.to_string(),
            })
            .collect();
        self.close();
        spawn_api(
            self,
            self.refresh,
            self.multiselect,
            ApiEffects {
                clear: true,
                bump: true,
            },
            api_bulk_rename(renames),
        );
    }

    pub(crate) fn set_rename_draft(self, v: String) {
        self.set_draft(|p| matches!(p, Pending::Rename { .. }), v);
    }

    pub(crate) fn set_new_lens_draft(self, v: String) {
        self.set_draft(|p| matches!(p, Pending::NewLens { .. }), v);
    }

    pub(crate) fn set_bulk_rename_draft(self, v: String) {
        self.set_draft(|p| matches!(p, Pending::BulkRename { .. }), v);
    }

    fn confirm_delete(self, paths: Vec<String>, lens_name: Option<String>) {
        self.pending
            .set(Some(Pending::DeleteConfirm { paths, lens_name }));
        self.selected.set(0);
    }

    fn copy_abs_path(self, root: Option<String>, path: &str, ok: &str) {
        self.close();
        let abs = absolute_path(root.as_deref(), path);
        copy_text(self, abs, ok);
    }

    fn bulk_paths_or_flash(self, ms: MultiselectCtx) -> Option<Vec<String>> {
        let paths = bulk_paths(ms);
        if paths.is_empty() {
            self.flash_warn("No rows selected");
            None
        } else {
            Some(paths)
        }
    }

    fn set_draft(self, matches: impl Fn(&Pending) -> bool, v: String) {
        self.pending.update(|p| {
            if let Some(pending) = p.as_mut()
                && matches(pending)
            {
                match pending {
                    Pending::Rename { draft, .. }
                    | Pending::NewLens { draft, .. }
                    | Pending::BulkRename { draft, .. } => *draft = v,
                    _ => {}
                }
            }
        });
    }
}

#[derive(Clone, Copy)]
struct ApiEffects {
    clear: bool,
    bump: bool,
}

impl ApiEffects {
    const fn flash_only() -> Self {
        Self {
            clear: false,
            bump: false,
        }
    }
}

fn spawn_api(
    ctx: SpaceMenuCtx,
    refresh: CatalogRefresh,
    ms: MultiselectCtx,
    effects: ApiEffects,
    fut: impl Future<Output = Result<String, String>> + 'static,
) {
    spawn_local(async move {
        match fut.await {
            Ok(msg) => {
                if effects.clear {
                    ms.clear();
                }
                if effects.bump {
                    refresh.bump();
                }
                ctx.flash(msg);
            }
            Err(e) => ctx.flash_err(e),
        }
    });
}

fn copy_text(ctx: SpaceMenuCtx, text: String, ok: &str) {
    let ok = ok.to_string();
    spawn_local(async move {
        match write_clipboard(&text).await {
            Ok(()) => ctx.flash(ok),
            Err(e) => ctx.flash_err(e),
        }
    });
}

async fn copy_zahir_json(ctx: SpaceMenuCtx, path: &str) {
    match fetch_entry_zahir_raw(path).await {
        Ok(Some(v)) => match serde_json::to_string_pretty(&v) {
            Ok(s) => match write_clipboard(&s).await {
                Ok(()) => ctx.flash("Copied Zahir JSON"),
                Err(e) => ctx.flash_err(e),
            },
            Err(e) => ctx.flash_err(e.to_string()),
        },
        Ok(None) => ctx.flash_warn("No Zahir JSON for this entry"),
        Err(e) => ctx.flash_err(e),
    }
}

async fn open_lens_picker(ctx: SpaceMenuCtx, paths: Vec<String>, exclude: Option<String>) {
    let mut names = fetch_lens_names().await;
    if let Some(ex) = &exclude {
        names.retain(|n| n != ex);
    }
    ctx.pending.set(Some(Pending::LensPicker {
        paths,
        names,
        exclude,
    }));
    ctx.selected.set(0);
    ctx.visible.set(true);
}
