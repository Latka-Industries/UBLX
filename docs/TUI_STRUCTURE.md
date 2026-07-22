# TUI structure map (for web port)

Source-of-truth map of the **live** ratatui chrome: bands, powerline **nodes**, and **where** each node sits (left vs right). Web must copy **placement**, not invent a dashboard.

**Code anchors**

| Layer | Path |
| ----- | ---- |
| Frame | [`src/render/core.rs`](../src/render/core.rs) — `draw_ublx_frame` |
| Modes / focus | [`src/layout/setup.rs`](../src/layout/setup.rs) — `MainMode`, `PanelFocus`, `RightPaneMode` |
| Powerline nodes | [`src/layout/style/nodes.rs`](../src/layout/style/nodes.rs) — `tab_node_segment`, `node_line`, `viewer_footer_line`, `status_node_spans` |
| Middle `current/total` | [`src/render/panes/middle.rs`](../src/render/panes/middle.rs) — `counter_line` / `line_for` → **`HorizontalAlignment::Right`** on panel `title_bottom` |
| Snapshot Contents | [`src/render/panes/snapshot_mode.rs`](../src/render/panes/snapshot_mode.rs) — same middle footer |
| Right Viewer footer | [`src/render/panes/right/chrome.rs`](../src/render/panes/right/chrome.rs) — size / mtime **right** |
| Tab order / labels | [`src/ui/consts/`](../src/ui/consts/) |
| Layout % | [`src/config/profile/core.rs`](../src/config/profile/core.rs) — `LayoutOverlay` |
| Web shell | [`crates/ublx-web/src/shell.rs`](../crates/ublx-web/src/shell.rs), [`panes.rs`](../crates/ublx-web/src/panes.rs) |

Related: [`WEB_UI.md`](WEB_UI.md).

---

## Vocabulary

| Term | Meaning in UBLX |
| ---- | --------------- |
| **Tab node** | Powerline pill `‹ label ›` (active / inactive styles). Used on the **main mode bar** and as **panel border titles**. |
| **Status / footer node** | Same pill look for chrome like `Last Snapshot`, `12/345`, size, mtime — **not** a second tab bar. |
| **`title` / `title_bottom`** | Ratatui `Block` title on the **top** border vs **bottom** border of a pane. Middle counter lives on **`title_bottom`**. |
| **Alignment** | Nodes are **left-** or **right-**aligned as a cluster inside that band. Do not put the file counter on the left of the middle pane. |

---

## Full-frame layout (every mode)

```text
┌──────────────────────────────────────────────────────────────────────────┐
│ ◀Snapshot▶ ◀Lenses?▶ ◀Delta▶ ◀Duplicates?▶ ◀Settings▶          ◀UBLX▶  │  TOP: main tabs (nodes L→R) + brand (RIGHT)
├──────────────────────────────────────────────────────────────────────────┤
│                    /absolute/path/to/indexed/root                        │  GAP: root path (CENTER, hint color)
├────────────┬─────────────────────────┬───────────────────────────────────┤
│            │                         │                                   │
│  LEFT      │  MIDDLE                 │  RIGHT                            │  BODY: 3-pane (or Settings 50/50)
│  pane      │  pane                   │  pane                             │
│            │                         │                                   │
│            │              …  ◀n/N▶   │                    ◀size▶◀mtime▶ │  ← pane border bottoms (see below)
├────────────┴─────────────────────────┴───────────────────────────────────┤
│ ◀Last Snapshot: …▶                                                       │  STATUS: snapshot node (LEFT)
│   — or —                                                                 │    OR catalog search strip (same slot)
│ ▌ Search (Categories & Contents): query…                                 │
└──────────────────────────────────────────────────────────────────────────┘
```

| Band | Height | Nodes / content | Horizontal placement |
| ---- | ------ | --------------- | -------------------- |
| Main tab row | 1 | Mode tab nodes + brand `UBLX` | Tabs **left → right**; brand **far right** (~4 cells) |
| Gap | 1 | Absolute indexed root | **Centered** |
| Body | rest − 1 | Mode panes | See per-mode |
| Status | 1 | `Last Snapshot: …` **or** catalog `/` search | Snapshot / search in the **left** status slot |

Constants: [`src/ui/consts/layout.rs`](../src/ui/consts/layout.rs).

Default `[layout]` widths: **left 10% / middle 30% / right 60%** (sum 100).

---

## Main tab row (nodes)

**Order (left → right):** Snapshot → Lenses (if any) → Delta → Duplicates (if any) → Settings → … → **`UBLX`** (brand, not a mode).

| Mode | Digit | When shown |
| ---- | ----- | ---------- |
| Snapshot | `1` | always |
| Lenses | `2` | ≥1 lens |
| Delta | `7` | always |
| Duplicates | `8` | non-empty groups; may read `Duplicates (H)` / `(N/S)` |
| Settings | `9` | always |

Cycle (`~`): Snapshot → Lenses? → Delta → Duplicates? → Settings → Snapshot.

Web: same labels as **in-app tabs** on `/` (optional `/?mode=`). Never use API paths (`/delta`, `/lenses`, …) as UI routes.

---

## Shared 3-pane body (Snapshot / Delta / Duplicates / Lenses)

Each pane is a bordered box. **Top border** = title tab node. **Bottom border** = optional footer nodes.

```text
┌─◀Categories▶────────────────┐┌─◀Contents▶ / ◀Paths▶──────────────┐┌─◀Viewer (v)▶ ◀Templates?▶ …──┐
│                             ││                                   ││                              │
│  list                       ││  path list                        ││  viewer / Zahir body          │
│                             ││                                   ││                              │
│                             ││                    [sort?] ◀n/N▶  ││              ◀size▶ ◀mtime▶  │
└─────────────────────────────┘└───────────────────────────────────┘└──────────────────────────────┘
     LEFT: title only                 MIDDLE: title_bottom          RIGHT Viewer: title_bottom
                                      cluster RIGHT-aligned           meta cluster RIGHT-aligned
```

### Placement rules (do not invent)

| Pane | Top (`title`) | Bottom (`title_bottom`) | Alignment of bottom cluster |
| ---- | ------------- | ----------------------- | --------------------------- |
| **Left** | Mode title node (`Categories`, `Delta type`, `Duplicate`, `Lens`) | *(none)* | — |
| **Middle** | `Contents` (Snapshot) or `Paths` (Delta / Dupes / Lenses) | **`current/total`** selection counter; Snapshot/Dupes may add **sort** node; Delta may add **Time** sort | **RIGHT** — counter is the **rightmost** node. Sort sits **immediately left** of the counter when present. Multiselect may append `· N sel` into the counter text. |
| **Right** | Tab nodes: `Viewer` / `Templates` / `Metadata` / `Writing` (hide empty) | Viewer only: size + mtime (+ PDF page when relevant) | **RIGHT** |
| **Right** (find) | — | Find strip on `title_bottom` when active | caller-specific (same strip family as status search) |

**Canonical counter code:** `middle::counter_line` → `style::node_line(..., HorizontalAlignment::Right, ...)`.  
**Canonical sort+counter:** `middle::line_for` → `viewer_footer_line(Some(counter), None, Some(sort), …)` so spans order is **`[sort][counter]`**, whole line right-aligned.

Empty lists: `(no contents)` / `(no matches)` (search). Loading placeholders in Delta-style modes when empty.

**Fullscreen viewer:** right pane can cover the body; status line still applies.

---

## Per-mode body

### Snapshot

| Pane | Top title | Body | Bottom |
| ---- | --------- | ---- | ------ |
| Left | `Categories` | `All` + category names | — |
| Middle | `Contents` | Paths for category (+ search); UBLX Settings paths may show as `Local` / `Global` | **RIGHT:** sort (`Name`/`Size`/`Mod` + arrow) + **`n/N`** |
| Right | Viewer tabs | Preview / Zahir for selection | **RIGHT:** size + mtime (Viewer tab) |

### Delta

| Pane | Top title | Body | Bottom |
| ---- | --------- | ---- | ------ |
| Left | `Delta type` | `Added` / `Modified` / `Removed` (delta colors) | — |
| Middle | `Paths` | Paths for type (+ search), often time-grouped | **RIGHT:** `Time ↕` sort + **`n/N`** |
| Right | `Snapshot overview` | Overview text — **not** file Viewer | — |

### Duplicates / Lenses

Same shell (`user_selected_mode`):

| Pane | Top | Body | Bottom |
| ---- | --- | ---- | ------ |
| Left | `Duplicate` / `Lens` | Groups / lens names | — |
| Middle | `Paths` | Member paths | **RIGHT:** sort (Dupes) + **`n/N`** (Lenses: counter only) |
| Right | Viewer tabs | Same as Snapshot | **RIGHT:** size + mtime |

### Settings (TUI: **not** 3-pane)

Full body is **50% / 50%** (ignores `[layout]`):

| Side | Content |
| ---- | ------- |
| Left | Scope tab nodes `Global` \| `Local` + option rows (bools, layout %, opacity, theme, …) |
| Right | Raw `ublx.toml` for active scope (scrollable buffer) |

**Web (locked):** still **3-pane** for Settings — scope · controls · help + **live read-only TOML** (no text editor). Writes: `GET`/`PATCH /settings/{scope}` structured JSON only. **Do not** expose TUI-only `bg_opacity` as a web control (terminal OSC chrome).

---

## Focus model

| Focus | Pane |
| ----- | ---- |
| `PanelFocus::Categories` | Left list |
| `PanelFocus::Contents` | Middle list |

Right pane is not a list-focus target. Focused pane: focused border + active title node; lists use highlight + list symbol (`›`).

---

## Status / overlays (port later)

| Surface | Slot | Notes |
| ------- | ---- | ----- |
| Catalog search (`/`) | **Status** (replaces Last Snapshot) | Filters categories + contents |
| Viewer find (Shift+S) | Right `title_bottom` | Literal find in right content |
| Help / theme / UBLX switch / Command Mode / space menus / toasts | Overlays | — |

---

## Web parity checklist

| Target | TUI truth | Web must match |
| ------ | --------- | -------------- |
| Middle counter | `title_bottom`, **right-aligned** `n/N` node | `PathsPane` footer → **`justify-content: flex-end`** (not left) |
| Middle sort | Optional node **left of** counter (Snapshot / Dupes / Delta) | Later |
| Right Viewer meta | size + mtime **right** | `right-pane-footer` already end-aligned |
| Status | Last Snapshot **left** | Shell footer |
| Settings | TUI 2-pane + editable-feeling TOML buffer | Web: controls + read-only live TOML |
| Embedded ship | — | `StaticMount::Embedded` later |
| Catalog search | status strip | wired (`/` / Esc / Enter; Skim fuzzy) | — |

---

## Label cheat sheet (`UI_STRINGS`)

- Tabs: Snapshot, Lenses, Delta, Duplicates, Settings  
- Snapshot: Categories, Contents, All, `(no contents)`, `(no matches)`  
- Delta: Delta type, Added, Modified, Removed, Snapshot overview  
- List modes: Duplicate, Lens, Paths  
- Right: Viewer, Templates, Metadata, Writing  
- Settings: Global, Local  
- Status: Last Snapshot, `Search (Categories & Contents): `
