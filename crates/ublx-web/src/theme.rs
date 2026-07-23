//! Apply server-provided `Palette` → CSS custom properties on `document.documentElement`.
//!
//! Favicon follows TUI brand chrome: page `background` + `title_brand` (same fields as
//! `--background` / `--brand`).

use std::collections::BTreeMap;

use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::api::ThemeCssBody;

/// Subset of settings `css` payload used by the shell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ThemeCssView {
    pub name: String,
    pub appearance: String,
    pub vars: BTreeMap<String, String>,
}

impl ThemeCssView {
    pub(crate) fn from_parts(
        name: String,
        appearance: String,
        vars: BTreeMap<String, String>,
    ) -> Self {
        Self {
            name,
            appearance,
            vars,
        }
    }

    pub(crate) fn from_body(body: &ThemeCssBody) -> Self {
        Self::from_parts(
            body.name.clone(),
            body.appearance.clone(),
            body.vars.clone(),
        )
    }
}

/// Write HSL tokens onto `:root`, toggle `html.dark`, and refresh the favicon.
pub(crate) fn apply_theme_css(css: &ThemeCssView) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(root) = doc.document_element() else {
        return;
    };
    let Ok(html) = root.dyn_into::<HtmlElement>() else {
        return;
    };

    let style = html.style();
    for (key, value) in &css.vars {
        let _ = style.set_property(key, value);
    }

    let classes = html.class_list();
    if css.appearance.eq_ignore_ascii_case("light") {
        let _ = classes.remove_1("dark");
    } else {
        let _ = classes.add_1("dark");
    }

    let _ = html.set_attribute("data-ublx-theme", &css.name);
    apply_favicon(&doc, &css.vars);
}

pub(crate) fn apply_theme_css_body(body: &ThemeCssBody) {
    apply_theme_css(&ThemeCssView::from_body(body));
}

/// TUI-shaped mark: rounded square in page bg, “U” in `title_brand`.
fn apply_favicon(doc: &web_sys::Document, vars: &BTreeMap<String, String>) {
    let Some(bg) = vars.get("--background") else {
        return;
    };
    let Some(brand) = vars.get("--brand") else {
        return;
    };
    let href = favicon_data_url(bg, brand);
    let Ok(Some(link)) = doc.query_selector("link[rel='icon']") else {
        return;
    };
    let _ = link.set_attribute("href", &href);
}

fn favicon_data_url(bg_hsl_token: &str, brand_hsl_token: &str) -> String {
    // Tokens are `"H S% L%"` (no hsl() wrapper) — same as shadcn CSS vars.
    let svg = format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 32 32\">",
            "<rect width=\"32\" height=\"32\" rx=\"4\" fill=\"hsl({bg})\"/>",
            "<text x=\"16\" y=\"26\" text-anchor=\"middle\" ",
            "font-family=\"ui-monospace,monospace\" font-size=\"30\" font-weight=\"900\" ",
            "stroke=\"hsl({fg})\" stroke-width=\"1.25\" paint-order=\"stroke fill\" ",
            "fill=\"hsl({fg})\">U</text>",
            "</svg>"
        ),
        bg = bg_hsl_token,
        fg = brand_hsl_token,
    );
    format!("data:image/svg+xml,{}", urlencoding::encode(&svg))
}
