//! Apply server-provided `Palette` → CSS custom properties on `document.documentElement`.

use std::collections::BTreeMap;

use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

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
}

/// Write HSL tokens onto `:root` and toggle `html.dark` for light vs dark palettes.
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
}
