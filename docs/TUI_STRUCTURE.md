# TUI structure map (for web port)

Source-of-truth map of the ratatui TUI chrome and per-mode panes. Use this when filling out [`crates/ublx-web`](../crates/ublx-web/) so Leptos mirrors roles and labels, not terminal key chords.

**Code anchors**

| Layer | Path |
| ----- | ---- |
| Frame entry | [`src/render/core.rs`](../src/render/core.rs) — `draw_ublx_frame` |
| Modes / focus | [`src/layout/setup.rs`](../src/layout/setup.rs) — `MainMode`, `PanelFocus`, `RightPaneMode` |
| Pane draws | [`src/render/panes/`](../src/render/panes/) |
| Tab order / labels | [`src/ui/consts/mod.rs`](../src/ui/consts/mod.rs), [`tabs.rs`](../src/ui/consts/tabs.rs), [`strings.rs`](../src/ui/consts/strings.rs) |
| Layout % | [`src/config/profile/core.rs`](../src/config/profile/core.rs) — `LayoutOverlay` |
| Web shell today | [`crates/ublx-web/src/shell.rs`](../crates/ublx-web/src/shell.rs), [`panes.rs`](../crates/ublx-web/src/panes.rs) |

Related design: [`WEB_UI.md`](WEB_UI.md).

---

## Vertical chrome (every mode)

Top → bottom, full terminal width:

```text
┌─────────────────────────────────────────────────────────────┐
│  [Snapshot] [Lenses?] [Delta] [Duplicates?] [Settings]  UBLX │  tab row
├─────────────────────────────────────────────────────────────┤
│              /absolute/path/to/indexed/root                 │  gap (hint)
├──────────────────┬──────────────────┬───────────────────────┤
│                  │                  │                       │
│   left pane      │   middle pane    │   right pane          │  body (3-pane
│                  │                  │                       │  or Settings)
│                  │                  │                       │
├──────────────────┴──────────────────┴───────────────────────┤
│  Last Snapshot: <ts>   |or|   Search (Categories & Contents) │  status
└─────────────────────────────────────────────────────────────┘
```

| Band | Height | Content |
| ----- | ------ | ------- |
| Tab row | 1 | Powerline tab nodes + brand `UBLX` (right, fixed ~4 cells) |
| Gap | 1 | Centered absolute indexed root (`hint` color); truncated middle if needed |
| Body | rest − 1 | Mode content (see below) |
| Status | 1 | `Last Snapshot: …` powerline node **or** catalog search strip (search replaces snapshot node while active / non-empty) |

Constants: [`src/ui/consts/layout.rs`](../src/ui/consts/layout.rs) (`tab_row_height`, `tab_body_gap_height`, `status_line_height`, `brand_block_width`).

Default 3-pane widths (`[layout]` in `ublx.toml`): **left 10% / middle 30% / right 60%**. Must sum to 100.

---

## Main tabs

**Visual order (left → right):** Snapshot → Lenses (if any) → Delta → Duplicates (if any) → Settings.

| Mode | Digit key | Shown when |
| ---- | --------- | ---------- |
| Snapshot | `1` | always |
| Lenses | `2` | at least one lens name |
| Delta | `7` | always (placeholder panes if no delta data) |
| Duplicates | `8` | non-empty duplicate groups; label may be `Duplicates (H)` or `Duplicates (N/S)` |
| Settings | `9` | always |

Cycle (`~` / MainModeToggle): Snapshot → Lenses? → Delta → Duplicates? → Settings → Snapshot.

Brand string: `UBLX` (not a tab).

---

## Focus model

| Focus | Meaning |
| ----- | ------- |
| `PanelFocus::Categories` | Left list |
| `PanelFocus::Contents` | Middle list |

Right pane is **not** a focus target for list navigation (read/scroll/viewer). Focused pane gets focused border + title node; lists use highlight style + list symbol.

---

## Shared 3-pane chrome

Used by Snapshot, Delta, Duplicates, Lenses (not Settings).

| Pane | Border title (default) | Body | Footer / extras |
| ---- | ---------------------- | ---- | --------------- |
| Left | Mode-specific (below) | Selectable list | — |
| Middle | `Contents` or `Paths` | Path list (virtualized ≥512 rows in Snapshot) | `current/total` counter; optional sort node; multiselect count |
| Right | Viewer tabs | Viewer: size / mtime footer (right-aligned); find strip on `title_bottom` |

Panel titles use the same powerline **tab-node** look as the main bar (active vs inactive).

Empty / loading: Delta-style placeholder (`—` / “Loading…”) when Duplicates/Lenses have no data.

**Fullscreen viewer:** Snapshot / Duplicates / Lenses can expand the right pane over the whole body (`viewer_fullscreen`); status line still applies.

---

## Per-mode panes

### Snapshot

| Pane | Title | Data |
| ---- | ----- | ---- |
| Left | `Categories` | `All` + filtered category names |
| Middle | `Contents` | Paths for selected category (+ search); UBLX Settings rows may display as `Local` / `Global` |
| Right | tabbed Viewer… | File preview / Zahir sections for selection |

Data flow: `load_snapshot_for_tui` → `ViewData` (category + search filter). See [`src/app/README.md`](../src/app/README.md).

Middle footer: selection counter + snapshot sort (`Name` / `Size` / `Time` + direction).

### Delta

| Pane | Title | Data |
| ---- | ----- | ---- |
| Left | `Delta type` | `Added` / `Modified` / `Removed` (delta palette colors) |
| Middle | paths list + counter | Paths for selected delta type (+ search) |
| Right | `Snapshot overview` | Scrollable overview text (`DeltaViewData.overview_text`) — **not** the file Viewer |

No selection-based `RightPaneContent` viewer in Delta.

### Duplicates

Same shell as Lenses (`user_selected_mode`):

| Pane | Title | Data |
| ---- | ----- | ---- |
| Left | `Duplicate` | Group labels (hash or name+size mode) |
| Middle | paths + counter | Member paths in selected group |
| Right | Viewer tabs | Same right-pane stack as Snapshot for selected path |

### Lenses

| Pane | Title | Data |
| ---- | ----- | ---- |
| Left | `Lens` | Lens names |
| Middle | paths + counter | Paths in selected lens |
| Right | Viewer tabs | Same as Snapshot |

### Settings (TUI: not 3-pane)

**50% / 50%** horizontal split over the full body (ignores `[layout]` percentages):

| Side | Content |
| ---- | ------- |
| Left | Scope powerline tabs `Global` \| `Local` + boolean / layout / opacity option rows |
| Right | Raw `ublx.toml` preview (scrollable) for the active scope |

**Web (locked):** no embedded TOML text editor. Edit via **controls only** (toggles, selects, steppers). Layout:

| Pane | Role |
| ---- | ---- |
| Left | Scope: `Global` / `Local` |
| Middle | Option rows as toggles / selects / value steppers (same keys as TUI settings rows) |
| Right | Help for the focused option + **live read-only TOML** (updates after each structured write) |

Writes go through `GET`/`PATCH /settings/{scope}` (structured JSON only — never a raw TOML body).

---

## Right pane (Snapshot / Duplicates / Lenses)

Tabs (hotkeys in TUI labels; web can omit chords):

| Mode | Label | Visibility |
| ---- | ----- | ---------- |
| Viewer | `Viewer (v)` | always |
| Templates | `Templates (t)` | non-empty `templates` |
| Metadata | `Metadata (m)` | `metadata` present |
| Writing | `Writing (w)` | `writing` present |

`RightPaneContent` carries viewer body, Zahir JSON sections, snap meta (size, mtime, category), and derived open/enhance flags. Viewer stack includes markdown, syntect, CSV/tables, images, PDF/video (optional tools), directory trees, etc. — full parity is **post**-v0.2.0 for web; start with text / Zahir JSON / meta.

---

## Status / search / overlays (port later)

Not required for first web fill-in, but part of the TUI surface:

| Surface | Role |
| ------- | ---- |
| Catalog search (`/`) | Filters categories + contents; replaces Last Snapshot on status line |
| Viewer find (Shift+S) | In-pane literal search on right content |
| Help (`?`) | Mode-aware key help overlay |
| Theme selector | Palette picker overlay |
| UBLX switch | Other indexed roots picker |
| Command Mode | `Ctrl+{leader}` chord menu |
| Space menus | File / lens / duplicate actions |
| Startup prompts | Root choice, previous settings, enhance-all |
| Toasts | Dev / notification |

---

## Web parity checklist (vs first-pass shell)

First pass already mirrors: main tabs (conditional Lenses/Delta/Duplicates), brand, project path gap, Last Snapshot footer, bordered 3-pane boxes, Snapshot categories + entries + right-pane tab shell.

| Target | TUI truth | Web today | Suggested next |
| ------ | --------- | --------- | -------------- |
| Delta left | Added / Modified / Removed | wired from `/delta` | — |
| Delta right | Snapshot overview text | snapshot timestamp list | — |
| Lenses | Lens → paths → Viewer | wired `/lenses` + members + detail | — |
| Duplicates | Group → paths → Viewer | wired `/duplicates` + detail | — |
| Snapshot right | Viewer / Templates / Metadata / Writing | entry detail + Zahir section tabs | Disk file Viewer body (later / optional) |
| Middle footer | `n/total` (+ sort) | Contents + list modes | Sort later; catalog search next |
| Catalog search | status-line search | missing | Filter client-side or query params |
| Settings | TUI: 2-pane + raw TOML | scope · controls · live TOML | — |
| Embedded ship | `StaticMount::Embedded` | `Dir` / `UBLX_WEB_DIST` | After UI modes feel real |

---

## Label cheat sheet

Copy these strings when wiring web titles (from `UI_STRINGS`):

- Tabs: Snapshot, Lenses, Delta, Duplicates, Settings  
- Snapshot: Categories, Contents, All, `(no contents)`, `(no matches)`  
- Delta: Delta type, Added, Modified, Removed, Snapshot overview  
- List modes: Duplicate, Lens, Paths  
- Right: Viewer, Templates, Metadata, Writing  
- Settings: Global, Local  
- Status: Last Snapshot, `Search (Categories & Contents): `
