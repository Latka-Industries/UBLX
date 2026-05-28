# KV tables

Rendering for the right-pane **Metadata** and **Writing** tabs: turn stored JSON (from zahirscan) into key/value and data tables with scroll.

Layout and scrollbar are handled by `render::scrollable_content`; here we parse JSON into sections and draw only the visible window (ratatui Table has no native scroll).

## Flow

1. **Input**: JSON string (metadata or writing_footprint) from `RightPaneContent`.
2. **Parse**: `sections::parse_json_sections` → list of `Section` (KeyValue, Contents, SingleColumnList).
3. **Draw**: `draw::draw_tables` slices to visible rows and renders tables in the given rect.

## Modules

| Module              | Purpose                                                                                                                                                                                                                                                                                                   |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **sections**        | Parse JSON blobs into `Section` (KeyValue, Contents, SingleColumnList). If a blob has `_metadata`, parsing unwraps that object first. The first KeyValue section keeps its own title when present; otherwise fallback title is “General”. Special keys: schema, sheet_stats, common_pivots, csv_metadata. |
| **walk**            | Map walk: root and nested objects → sections (flat KV, schema, sheet_stats, common_pivots, csv_metadata, entries).                                                                                                                                                                                        |
| **column_metadata** | Compact `columns` stats → typed tables (e.g. “Number columns”); honors [`typed_column_tables`][typed-column-tables]. Section titles for nested compact metadata are prefixed with the parent table title (`parent · …`). Stale parallel-array JSON → notice to clear `.ublx` / cache and re-scan. |
| **xlsx**            | XLSX: sheet_stats (rows/columns per sheet) → table.                                                                                                                                                                                                                                                       |
| **schema**          | Schema tree section from JSON.                                                                                                                                                                                                                                                                            |
| **format**          | Key/value and value display formatting.                                                                                                                                                                                                                                                                   |
| **draw**            | `draw_tables(area, json, scroll_y)`: layout sections, slice to viewport, render.                                                                                                                                                                                                                          |
| **consts**          | Section key names, table gap.                                                                                                                                                                                                                                                                             |

Used from `render::panes::right::draw` (scrollable body) when `right_pane_mode` is Metadata or Writing.

[typed-column-tables]: ../../../README.md#configuration
