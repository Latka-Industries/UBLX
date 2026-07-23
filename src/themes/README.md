# Themes

Theme names match `Palette::name` in `palettes.rs`: use them as `theme = "..."` in `ublx.toml` or `.ublx.toml`. The in-app theme picker (**Command Mode + t**) previews and can save the choice.

**Web UI:** palettes are not a free-form design kit. The TUI’s [`layout/style`](../layout/style/) (`ThemeStyles` / `tab_active`, etc.) defines which fields are used together. The web port exports those same fields as CSS vars (`css.rs`) and must keep the same pairings — see [`docs/WEB_UI.md`](../../docs/WEB_UI.md) § Theming.

## Picker order

The picker shows **Dark** and **Light** section headers. Under each, themes are sorted **A–Z** by display name (same order as `theme_selector_entries` / `theme_ordered_list` in `mod.rs`). The tables below match that order.

**Adding a theme:** define a `pub static` `Palette` in `palettes.rs`, append it to `ALL_THEMES`, and set `Palette::name` to the exact display name (as in the tables).

## Allowable values

### Dark

| Theme name             | Description                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------------- |
| **Archival Simulacra** | True black page, neon green body text, bright green focus and tabs, dim emerald hints — matrix-style.         |
| **Babel Blend**        | Deep navy page, warm parchment body text, orange focus/search, brick-red active tabs, coral hints.            |
| **Burning Glyph**      | Maroon-black page, pale buttery text, coral-red focus and search, amber brand.                                |
| **Frozen Phrase**      | Nordic blue-gray page, snow text, frost-blue focus, ice-pale active tabs, sea-glass search, muted hints.      |
| **Garden Unseen**      | Deep forest-green page, peach-cream text, mint focus and search, olive-brown brand.                           |
| **Golden Delirium**    | Olive-black page, soft pink/cream text, yellow-lime focus and search, rust-brown brand.                       |
| **Oblivion Ink**       | **Default.** Deep navy page, pale aqua text, cyan focus and search, magenta hints and brand.                  |
| **Purple Haze**        | Near-black violet page, lavender-rose text, magenta focus and search, violet brand.                           |
| **Resin Record**       | Near-black page, warm amber body text, amber focus/search — compact “CRT” feel.                               |
| **Shadow Index**       | Near-black page, cool off-white text, medium-gray focus, hints with a slight blue-gray cast (not a light UI). |
| **Tangerine Memory**   | Burnt umber page, honey-cream text, peach-gold focus and search, dusty rose brand.                            |

### Light

| Theme name        | Description                                                                                                       |
| ----------------- | ----------------------------------------------------------------------------------------------------------------- |
| **Asterion Code** | Cool blue-gray page, blue-forward body text, teal focus, warm clay hints (distinct from Parched Page).            |
| **Barley Bound**  | Buttercream page, warm dark body text, teal focus, olive search, orange brand, stone hints (Gruvbox-light–style). |
| **Cryptic Chai**  | Tea-stained parchment, dark chocolate text, copper-brown focus and tabs, muted hints.                             |
| **Faded Echo**    | Dusty sepia paper, book-ink text, copper-brown accents, archival calm.                                            |
| **Infinite Rose** | Pale cool-gray page, rose/mauve body text and chrome, dusty hints.                                                |
| **Obdurate Noon** | Solarized-light–style parchment, muted blue-gray text, cyan focus, blue tabs/search, violet hints.                |
| **Ochre Thread**  | Pale sand page, burnt-orange text, copper focus and rust tabs, blue-gray hints.                                   |
| **Pale Mirror**   | Frosted blue-lilac page, plum body text, purple tab/focus chrome, rose-mauve hints.                               |
| **Parched Page**  | Warm cream page, forest-green text and green tab chrome, amber-brown hints.                                       |
| **Silent Sheet**  | White page, black text, charcoal focus, slate-blue active tabs, warm gray hints.                                  |
| **Verglas Trace** | Snow page, polar-night body text, frost-blue focus/search, slate tabs, icy brand (Nord-light–style).              |

Omit `theme` or set `theme = "default"` to use **Oblivion Ink** (`DEFAULT_THEME` in `mod.rs`).
